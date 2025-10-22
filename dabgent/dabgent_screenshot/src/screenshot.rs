use crate::playwright::build_playwright_base;
use crate::types::{ScreenshotError, ScreenshotOptions};
use dagger_sdk::{DaggerConn, Directory, Service};
use eyre::Result;

/// Capture a screenshot of a running web service
pub async fn screenshot_service(
    client: &DaggerConn,
    service: Service,
    options: ScreenshotOptions,
) -> Result<Directory, ScreenshotError> {
    tracing::info!("Starting screenshot capture for service");

    let playwright_base = build_playwright_base(client)
        .await
        .map_err(|e| ScreenshotError::PlaywrightBuildFailed(e.to_string()))?;

    let url = options.url.unwrap_or_else(|| "/".to_string());

    tracing::debug!(
        "Configuring Playwright: url={}, port={}, wait_time={}ms",
        url,
        options.port,
        options.wait_time_ms
    );

    let container = playwright_base
        .with_service_binding("app", service)
        .with_env_variable("TARGET_URL", url)
        .with_env_variable("TARGET_PORT", options.port.to_string())
        .with_env_variable("WAIT_TIME", options.wait_time_ms.to_string())
        .with_env_variable("CACHE_BUST", chrono::Utc::now().timestamp().to_string())
        .with_exec(vec![
            "npx",
            "playwright",
            "test",
            "--config=playwright.single.config.ts",
        ]);

    tracing::info!("Executing screenshot capture");

    let screenshots_dir = container.directory("/screenshots");

    Ok(screenshots_dir)
}

/// Build and screenshot an app from a directory with a Dockerfile
pub async fn screenshot_app(
    client: &DaggerConn,
    app_source: Directory,
    options: ScreenshotOptions,
) -> Result<Directory, ScreenshotError> {
    tracing::info!("Building app from Dockerfile");

    // exclude node_modules and .git
    // Note: dagger-sdk doesn't have exclude patterns, so we use the source as-is
    // The Dockerfile should handle exclusions via .dockerignore
    let filtered_source = app_source;

    // build container from Dockerfile
    let mut app_container = filtered_source.docker_build();

    // parse and apply environment variables
    for (key, value) in &options.env_vars {
        tracing::debug!("Setting env var: {}={}", key, value);
        app_container = app_container.with_env_variable(key, value);
    }

    // force evaluation to catch build errors early
    app_container
        .sync()
        .await
        .map_err(|e| ScreenshotError::AppBuildFailed(e.to_string()))?;

    tracing::info!("App build successful, starting service");

    let service = app_container.with_exposed_port(options.port as isize).as_service();

    screenshot_service(client, service, options).await
}

/// Screenshot multiple apps in batch with controlled concurrency
pub async fn screenshot_apps_batch(
    client: &DaggerConn,
    app_sources: Vec<Directory>,
    options: ScreenshotOptions,
    concurrency: usize,
) -> Result<Directory, ScreenshotError> {
    tracing::info!(
        "Starting batch screenshot for {} apps with concurrency {}",
        app_sources.len(),
        concurrency
    );

    // build and validate app containers with controlled concurrency
    let mut services = Vec::new();
    let mut handles: Vec<tokio::task::JoinHandle<Result<Option<(usize, Service)>, ScreenshotError>>> = Vec::new();

    for (i, app_source) in app_sources.into_iter().enumerate() {
        let client_clone = client.clone();
        let options_clone = options.clone();

        let handle = tokio::spawn(async move {
            tracing::info!("[app-{}] Building container", i);

            // Note: dagger-sdk doesn't have exclude patterns, so we use the source as-is
            let filtered_source = app_source;

            let mut app_container = filtered_source.docker_build();

            for (key, value) in &options_clone.env_vars {
                app_container = app_container.with_env_variable(key, value);
            }

            // validate build
            match app_container.sync().await {
                Ok(_) => {
                    tracing::info!("[app-{}] Build successful", i);

                    let service = app_container
                        .with_exposed_port(options_clone.port as isize)
                        .as_service();

                    // test service startup
                    let test_result = client_clone
                        .container()
                        .from("alpine:3.18")
                        .with_service_binding("test-app", service.clone())
                        .with_exec(vec!["sh", "-c", "sleep 3"])
                        .sync()
                        .await;

                    match test_result {
                        Ok(_) => {
                            tracing::info!("[app-{}] Service starts successfully", i);
                            Ok(Some((i, service)))
                        }
                        Err(e) => {
                            tracing::error!("[app-{}] Service failed to start: {}", i, e);
                            Ok(None)
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("[app-{}] Build failed: {}", i, e);
                    Ok(None)
                }
            }
        });

        handles.push(handle);

        // limit concurrency
        if handles.len() >= concurrency {
            let result = handles.remove(0).await.map_err(|e| {
                ScreenshotError::Other(eyre::eyre!("Task join error: {}", e))
            })??;
            if let Some(service) = result {
                services.push(service);
            }
        }
    }

    // wait for remaining tasks
    for handle in handles {
        let result = handle.await.map_err(|e| {
            ScreenshotError::Other(eyre::eyre!("Task join error: {}", e))
        })??;
        if let Some(service) = result {
            services.push(service);
        }
    }

    let num_services = services.len();
    tracing::info!("Successfully built {} apps", num_services);

    // build playwright container and bind all services
    let playwright_base = build_playwright_base(client)
        .await
        .map_err(|e| ScreenshotError::PlaywrightBuildFailed(e.to_string()))?;

    let mut playwright_container = playwright_base;
    for (i, service) in services {
        playwright_container = playwright_container.with_service_binding(&format!("app-{}", i), service);
    }

    let url = options.url.unwrap_or_else(|| "/".to_string());

    playwright_container = playwright_container
        .with_env_variable("TARGET_URL", url)
        .with_env_variable("TARGET_PORT", options.port.to_string())
        .with_env_variable("WAIT_TIME", options.wait_time_ms.to_string())
        .with_env_variable("CONCURRENCY", concurrency.to_string())
        .with_env_variable("NUM_APPS", num_services.to_string())
        .with_env_variable("CACHE_BUST", chrono::Utc::now().timestamp().to_string())
        .with_exec(vec![
            "npx",
            "playwright",
            "test",
            "--config=playwright.batch.config.ts",
        ]);

    tracing::info!("Executing batch screenshot capture");

    Ok(playwright_container.directory("/screenshots"))
}

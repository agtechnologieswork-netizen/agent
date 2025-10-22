use crate::playwright::build_playwright_base;
use crate::types::{ScreenshotError, ScreenshotOptions};
use dagger_sdk::{DaggerConn, Directory, Service};
use eyre::{Context, Result};
use std::time::{SystemTime, UNIX_EPOCH};

/// Build an app service from source directory
async fn build_app_service(
    app_source: Directory,
    options: &ScreenshotOptions,
) -> Result<Service> {
    let mut app_container = app_source.docker_build();

    for (key, value) in &options.env_vars {
        tracing::debug!("Setting env var: {}={}", key, value);
        app_container = app_container.with_env_variable(key, value);
    }

    app_container
        .sync()
        .await
        .context("failed to build app container")?;

    Ok(app_container.with_exposed_port(options.port as isize).as_service())
}

/// Capture a screenshot of a running web service
pub async fn screenshot_service(
    client: &DaggerConn,
    service: Service,
    options: ScreenshotOptions,
) -> Result<Directory, ScreenshotError> {
    tracing::info!("Starting screenshot capture for service");

    let playwright_base = build_playwright_base(client)
        .await
        .context("failed to build playwright container")?;

    tracing::debug!(
        "Configuring Playwright: url={}, port={}, wait_time={}ms",
        options.url,
        options.port,
        options.wait_time_ms
    );

    let cache_bust = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let container = playwright_base
        .with_service_binding("app", service)
        .with_env_variable("TARGET_URL", &options.url)
        .with_env_variable("TARGET_PORT", options.port.to_string())
        .with_env_variable("WAIT_TIME", options.wait_time_ms.to_string())
        .with_env_variable("CACHE_BUST", cache_bust)
        .with_exec(vec![
            "npx",
            "playwright",
            "test",
            "--config=playwright.single.config.ts",
        ]);

    tracing::info!("Executing screenshot capture");

    Ok(container.directory("/screenshots"))
}

/// Build and screenshot an app from a directory with a Dockerfile
pub async fn screenshot_app(
    client: &DaggerConn,
    app_source: Directory,
    options: ScreenshotOptions,
) -> Result<Directory, ScreenshotError> {
    tracing::info!("Building app from Dockerfile");

    let service = build_app_service(app_source, &options)
        .await
        .context("failed to build app service")?;

    tracing::info!("App build successful, starting service");

    screenshot_service(client, service, options).await
}

/// Screenshot multiple apps in batch with controlled concurrency
pub async fn screenshot_apps_batch(
    client: &DaggerConn,
    app_sources: Vec<Directory>,
    options: ScreenshotOptions,
    concurrency: usize,
) -> Result<Directory, ScreenshotError> {
    use futures::stream::{FuturesUnordered, StreamExt};

    tracing::info!(
        "Starting batch screenshot for {} apps with concurrency {}",
        app_sources.len(),
        concurrency
    );

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut futures = FuturesUnordered::new();

    for (i, app_source) in app_sources.into_iter().enumerate() {
        let options_clone = options.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();

        futures.push(async move {
            let _permit = permit;
            tracing::info!("[app-{}] Building container", i);

            match build_app_service(app_source, &options_clone).await {
                Ok(service) => {
                    tracing::info!("[app-{}] Build successful", i);
                    Some((i, service))
                }
                Err(e) => {
                    tracing::error!("[app-{}] Build failed: {}", i, e);
                    None
                }
            }
        });
    }

    let mut services = Vec::new();
    while let Some(result) = futures.next().await {
        if let Some(service) = result {
            services.push(service);
        }
    }

    let num_services = services.len();
    tracing::info!("Successfully built {} apps", num_services);

    let playwright_base = build_playwright_base(client)
        .await
        .context("failed to build playwright container")?;

    let mut playwright_container = playwright_base;
    for (i, service) in services {
        playwright_container =
            playwright_container.with_service_binding(&format!("app-{}", i), service);
    }

    let cache_bust = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    playwright_container = playwright_container
        .with_env_variable("TARGET_URL", &options.url)
        .with_env_variable("TARGET_PORT", options.port.to_string())
        .with_env_variable("WAIT_TIME", options.wait_time_ms.to_string())
        .with_env_variable("CONCURRENCY", concurrency.to_string())
        .with_env_variable("NUM_APPS", num_services.to_string())
        .with_env_variable("CACHE_BUST", cache_bust)
        .with_exec(vec![
            "npx",
            "playwright",
            "test",
            "--config=playwright.batch.config.ts",
        ]);

    tracing::info!("Executing batch screenshot capture");

    Ok(playwright_container.directory("/screenshots"))
}

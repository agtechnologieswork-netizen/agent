use crate::playwright::build_playwright_base;
use crate::types::ScreenshotOptions;
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

    let port = options.port as isize;
    Ok(app_container.with_exposed_port(port).as_service())
}

/// Capture a screenshot of a running web service
pub async fn screenshot_service(
    client: &DaggerConn,
    service: Service,
    options: ScreenshotOptions,
) -> Result<Directory> {
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
        .context("system time before UNIX_EPOCH")?
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

    // force execution before returning directory
    container.sync().await.context("failed to execute playwright tests")?;

    Ok(container.directory("/screenshots"))
}

/// Build and screenshot an app from a directory with a Dockerfile
pub async fn screenshot_app(
    client: &DaggerConn,
    app_source: Directory,
    options: ScreenshotOptions,
) -> Result<Directory> {
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
) -> Result<Directory> {
    use futures::stream::{FuturesUnordered, StreamExt};

    let total_apps = app_sources.len();
    tracing::info!(
        "Starting batch screenshot for {} apps with concurrency {}",
        total_apps,
        concurrency
    );

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut futures = FuturesUnordered::new();

    for (i, app_source) in app_sources.into_iter().enumerate() {
        let options_clone = options.clone();
        let semaphore_clone = semaphore.clone();

        futures.push(async move {
            let _permit = semaphore_clone
                .acquire_owned()
                .await
                .expect("semaphore should not be closed");
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
        if let Some((i, service)) = result {
            services.push((i, service));
        }
    }

    let num_services = services.len();
    let build_failures = total_apps - num_services;

    if build_failures > 0 {
        tracing::warn!("Build phase: {} succeeded, {} failed", num_services, build_failures);
    } else {
        tracing::info!("Build phase: all {} apps built successfully", num_services);
    }

    let playwright_base = build_playwright_base(client)
        .await
        .context("failed to build playwright container")?;

    // validate services can start by checking their endpoints
    // services that fail this check will be filtered out before binding to playwright
    tracing::info!("Validating {} services", num_services);

    let mut valid_services = Vec::new();
    let mut service_check_failures = 0;

    for (i, service) in services {
        // attempt to check if service can start by creating a test container that connects to it
        let check_container = client
            .container()
            .from("alpine:latest")
            .with_service_binding(&format!("app-{}", i), service.clone())
            .with_exec(vec!["true"]);

        match check_container.sync().await {
            Ok(_) => {
                tracing::debug!("[app-{}] Service validation passed", i);
                valid_services.push((i, service));
            }
            Err(e) => {
                tracing::warn!("[app-{}] Service validation failed: {}", i, e);
                service_check_failures += 1;
            }
        }
    }

    if service_check_failures > 0 {
        tracing::warn!(
            "Service validation: {} passed, {} failed",
            valid_services.len(),
            service_check_failures
        );
    }

    if valid_services.is_empty() {
        return Err(eyre::eyre!(
            "All services failed validation. Check that apps are listening on port {} and starting successfully.",
            options.port
        ));
    }

    // bind all valid services to playwright container
    let mut playwright_container = playwright_base;
    for (i, service) in &valid_services {
        playwright_container =
            playwright_container.with_service_binding(&format!("app-{}", i), service.clone());
    }

    let cache_bust = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX_EPOCH")?
        .as_secs()
        .to_string();

    playwright_container = playwright_container
        .with_env_variable("TARGET_URL", &options.url)
        .with_env_variable("TARGET_PORT", options.port.to_string())
        .with_env_variable("WAIT_TIME", options.wait_time_ms.to_string())
        .with_env_variable("CONCURRENCY", concurrency.to_string())
        .with_env_variable("NUM_APPS", valid_services.len().to_string())
        .with_env_variable("CACHE_BUST", cache_bust)
        .with_exec(vec![
            "npx",
            "playwright",
            "test",
            "--config=playwright.batch.config.ts",
        ]);

    tracing::info!("Executing batch screenshot capture for {} services", valid_services.len());

    playwright_container.sync().await.context("failed to execute playwright tests")?;

    let screenshots_dir = playwright_container.directory("/screenshots");

    // TODO: parse summary.json to report per-app screenshot results
    tracing::info!(
        "Total: {} apps processed, {} build failures, {} service validation failures",
        total_apps,
        build_failures,
        service_check_failures
    );

    Ok(screenshots_dir)
}

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
    // check for Dockerfile existence first
    let dockerfile_exists = app_source
        .file("Dockerfile")
        .sync()
        .await
        .is_ok();

    if !dockerfile_exists {
        eyre::bail!("Dockerfile not found in app source directory");
    }

    let mut app_container = app_source.docker_build();

    for (key, value) in &options.env_vars {
        tracing::debug!("Setting env var: {}={}", key, value);
        app_container = app_container.with_env_variable(key, value);
    }

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
    app_sources: Vec<(String, Directory)>,
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

    for (i, (path, app_source)) in app_sources.into_iter().enumerate() {
        let options_clone = options.clone();
        let semaphore_clone = semaphore.clone();

        futures.push(async move {
            let _permit = semaphore_clone
                .acquire_owned()
                .await
                .expect("semaphore should not be closed");
            tracing::info!("[app-{}] Building container from {}", i, path);

            match build_app_service(app_source, &options_clone).await {
                Ok(service) => {
                    tracing::info!("[app-{}] Build successful", i);
                    Some((i, service))
                }
                Err(e) => {
                    tracing::error!("[app-{}] Build failed ({}): {}", i, path, e);
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

    // screenshot services individually with controlled concurrency
    // this approach is more robust - if one service crashes, others continue
    tracing::info!("Starting screenshot capture for {} services", num_services);

    let playwright_base = build_playwright_base(client)
        .await
        .context("failed to build playwright container")?;

    let cache_bust = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX_EPOCH")?
        .as_secs()
        .to_string();

    // use a shared playwright base but create separate containers for each service
    // this allows us to handle service failures gracefully
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut screenshot_futures = FuturesUnordered::new();

    for (i, service) in services {
        let playwright_clone = playwright_base.clone();
        let options_clone = options.clone();
        let cache_bust_clone = cache_bust.clone();
        let semaphore_clone = semaphore.clone();

        screenshot_futures.push(async move {
            let _permit = semaphore_clone
                .acquire_owned()
                .await
                .expect("semaphore should not be closed");

            tracing::info!("[app-{}] Screenshotting", i);

            // bind service and run playwright for this specific app
            let container = playwright_clone
                .with_service_binding("app", service)
                .with_env_variable("TARGET_URL", &options_clone.url)
                .with_env_variable("TARGET_PORT", options_clone.port.to_string())
                .with_env_variable("WAIT_TIME", options_clone.wait_time_ms.to_string())
                .with_env_variable("CACHE_BUST", &cache_bust_clone)
                .with_exec(vec![
                    "npx",
                    "playwright",
                    "test",
                    "--config=playwright.single.config.ts",
                ]);

            // execute and capture result
            match container.sync().await {
                Ok(_) => {
                    tracing::info!("[app-{}] Screenshot captured", i);
                    Some((i, container.directory("/screenshots")))
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("exit code: 1") || error_msg.contains("no such host") {
                        tracing::warn!("[app-{}] Service crashed on startup", i);
                    } else {
                        tracing::error!("[app-{}] Screenshot failed: {}", i, e);
                    }
                    None
                }
            }
        });
    }

    // collect results
    let mut screenshot_results = Vec::new();
    while let Some(result) = screenshot_futures.next().await {
        if let Some((i, dir)) = result {
            screenshot_results.push((i, dir));
        }
    }

    let screenshot_count = screenshot_results.len();
    let screenshot_failures = num_services - screenshot_count;

    // merge all successful screenshots into output directory
    let mut output_dir = client.directory();
    for (i, dir) in screenshot_results {
        output_dir = output_dir.with_directory(&format!("app-{}", i), dir);
    }

    tracing::info!(
        "Screenshot phase: {} succeeded, {} failed",
        screenshot_count,
        screenshot_failures
    );

    tracing::info!(
        "Total: {} apps, {} screenshots, {} failed (build: {}, screenshot: {})",
        total_apps,
        screenshot_count,
        build_failures + screenshot_failures,
        build_failures,
        screenshot_failures
    );

    Ok(output_dir)
}

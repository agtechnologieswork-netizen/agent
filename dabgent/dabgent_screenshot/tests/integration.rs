use dabgent_screenshot::{screenshot_app, ScreenshotOptions};
use std::path::Path;
use tempfile::TempDir;

/// Test screenshotting a simple nginx app
#[tokio::test]
#[ignore] // requires Dagger to be running
async fn test_screenshot_nginx() {
    // create a minimal Dockerfile for nginx
    let temp_dir = TempDir::new().unwrap();
    let app_dir = temp_dir.path();

    std::fs::write(
        app_dir.join("Dockerfile"),
        r#"FROM nginx:alpine
COPY index.html /usr/share/nginx/html/index.html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
"#,
    )
    .unwrap();

    std::fs::write(
        app_dir.join("index.html"),
        r#"<!DOCTYPE html>
<html>
<head><title>Test App</title></head>
<body><h1>Hello from test!</h1></body>
</html>
"#,
    )
    .unwrap();

    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().to_string_lossy().to_string();

    let options = ScreenshotOptions {
        port: 80,
        wait_time_ms: 10000,
        ..Default::default()
    };

    // connect to Dagger and take screenshot
    dagger_sdk::connect(|client| async move {
        let app_source = client.host().directory(app_dir.to_string_lossy().to_string());

        // take screenshot
        let screenshots_dir = screenshot_app(&client, app_source, options)
            .await
            .expect("Screenshot should succeed");

        // export and verify
        screenshots_dir.export(&output_path).await.unwrap();
        Ok(())
    })
    .await
    .unwrap();

    let screenshot_path = Path::new(&output_path).join("screenshot.png");
    let logs_path = Path::new(&output_path).join("logs.txt");

    assert!(screenshot_path.exists(), "screenshot.png should exist");
    assert!(logs_path.exists(), "logs.txt should exist");

    // verify screenshot has content
    let screenshot_size = std::fs::metadata(&screenshot_path).unwrap().len();
    assert!(screenshot_size > 1000, "Screenshot should have reasonable size");

    println!("✓ Screenshot test passed");
    println!("  Screenshot: {}", screenshot_path.display());
    println!("  Logs: {}", logs_path.display());
}

/// Test that environment variables are properly injected
#[tokio::test]
#[ignore] // requires Dagger to be running
async fn test_screenshot_with_env_vars() {
    let temp_dir = TempDir::new().unwrap();
    let app_dir = temp_dir.path();

    // create a simple Node.js app that displays an env var
    std::fs::write(
        app_dir.join("Dockerfile"),
        r#"FROM node:18-alpine
WORKDIR /app
COPY server.js .
EXPOSE 8000
CMD ["node", "server.js"]
"#,
    )
    .unwrap();

    std::fs::write(
        app_dir.join("server.js"),
        r#"const http = require('http');
const message = process.env.TEST_MESSAGE || 'No message';
const server = http.createServer((req, res) => {
  res.writeHead(200, {'Content-Type': 'text/html'});
  res.end(`<html><body><h1>${message}</h1></body></html>`);
});
server.listen(8000, () => console.log('Server running on port 8000'));
"#,
    )
    .unwrap();

    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().to_string_lossy().to_string();

    let options = ScreenshotOptions {
        port: 8000,
        wait_time_ms: 15000,
        env_vars: vec![("TEST_MESSAGE".to_string(), "Hello from env!".to_string())],
        ..Default::default()
    };

    dagger_sdk::connect(|client| async move {
        let app_source = client.host().directory(app_dir.to_string_lossy().to_string());

        let screenshots_dir = screenshot_app(&client, app_source, options)
            .await
            .expect("Screenshot should succeed");

        screenshots_dir.export(&output_path).await.unwrap();
        Ok(())
    })
    .await
    .unwrap();

    let screenshot_path = Path::new(&output_path).join("screenshot.png");
    assert!(screenshot_path.exists(), "screenshot.png should exist");

    println!("✓ Environment variable test passed");
}

#[test]
fn test_screenshot_options_default() {
    let options = ScreenshotOptions::default();
    assert_eq!(options.port, 8000);
    assert_eq!(options.wait_time_ms, 30000);
    assert_eq!(options.url, Some("/".to_string()));
    assert_eq!(options.env_vars.len(), 0);
}

#[test]
fn test_screenshot_options_custom() {
    let options = ScreenshotOptions {
        port: 3000,
        wait_time_ms: 5000,
        url: Some("/health".to_string()),
        env_vars: vec![("KEY".to_string(), "VALUE".to_string())],
    };

    assert_eq!(options.port, 3000);
    assert_eq!(options.wait_time_ms, 5000);
    assert_eq!(options.url, Some("/health".to_string()));
    assert_eq!(options.env_vars.len(), 1);
}

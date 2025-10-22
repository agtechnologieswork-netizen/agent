use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotOptions {
    /// URL path to navigate to (default: "/")
    pub url: Option<String>,
    /// Port the service listens on (default: 8000)
    pub port: u16,
    /// Maximum timeout to wait for network idle in ms (default: 30000)
    pub wait_time_ms: u64,
    /// Environment variables to inject into the app container
    pub env_vars: Vec<(String, String)>,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            url: Some("/".to_string()),
            port: 8000,
            wait_time_ms: 30000,
            env_vars: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScreenshotResult {
    /// Path to the screenshot file
    pub screenshot_path: String,
    /// Path to the browser logs file
    pub logs_path: String,
    /// Whether the screenshot was successful
    pub success: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ScreenshotError {
    #[error("Failed to build Playwright container: {0}")]
    PlaywrightBuildFailed(String),

    #[error("Failed to build app container: {0}")]
    AppBuildFailed(String),

    #[error("Screenshot capture failed: {0}")]
    CaptureFailed(String),

    #[error("Dagger error: {0}")]
    DaggerError(#[from] dagger_sdk::errors::DaggerError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(#[from] eyre::Error),
}

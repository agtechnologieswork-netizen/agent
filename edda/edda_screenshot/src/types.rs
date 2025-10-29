use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotOptions {
    /// URL path to navigate to (default: "/")
    pub url: String,
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
            url: "/".to_string(),
            port: 8000,
            wait_time_ms: 30000,
            env_vars: vec![],
        }
    }
}

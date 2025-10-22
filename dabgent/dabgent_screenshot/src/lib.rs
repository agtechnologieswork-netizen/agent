pub mod playwright;
pub mod screenshot;
pub mod types;

pub use screenshot::{screenshot_app, screenshot_apps_batch, screenshot_service};
pub use types::{ScreenshotError, ScreenshotOptions, ScreenshotResult};

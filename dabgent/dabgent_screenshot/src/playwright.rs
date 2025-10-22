use dagger_sdk::{Container, DaggerConn, Directory};
use eyre::Result;
use include_dir::{include_dir, Dir};

const PLAYWRIGHT_VERSION: &str = "v1.40.0-jammy";

// Embed playwright directory at compile time
static PLAYWRIGHT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/playwright");

/// Build the base Playwright container with cached dependencies
pub async fn build_playwright_base(client: &DaggerConn) -> Result<Container> {
    let playwright_source = get_playwright_source(client)?;

    tracing::info!("Building Playwright base container");

    let container = client
        .container()
        .from(format!("mcr.microsoft.com/playwright:{}", PLAYWRIGHT_VERSION))
        .with_workdir("/tests")
        .with_directory("/tests", playwright_source)
        .with_exec(vec!["npm", "install"])
        .with_mounted_cache("/ms-playwright", client.cache_volume("playwright-browsers"))
        .with_exec(vec!["npx", "playwright", "install", "chromium"]);

    Ok(container)
}

/// Get the Playwright source directory by extracting embedded files to temp directory
fn get_playwright_source(client: &DaggerConn) -> Result<Directory> {
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path();

    tracing::debug!("Extracting embedded Playwright files to: {:?}", temp_path);

    // Extract all files from embedded directory
    extract_dir(&PLAYWRIGHT_DIR, temp_path)?;

    // Create Dagger directory from temp path
    let playwright_dir = client
        .host()
        .directory(temp_path.to_string_lossy().to_string());

    // Keep temp_dir alive for the duration of this operation
    // Dagger will read files from it before it's dropped
    std::mem::forget(temp_dir);

    Ok(playwright_dir)
}

/// Recursively extract embedded directory to filesystem
fn extract_dir(dir: &include_dir::Dir, target_path: &std::path::Path) -> Result<()> {
    use std::fs;

    // Create target directory
    fs::create_dir_all(target_path)?;

    // Extract all files
    for file in dir.files() {
        let file_path = target_path.join(file.path());
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, file.contents())?;
        tracing::trace!("Extracted: {:?}", file_path);
    }

    // Extract all subdirectories
    for subdir in dir.dirs() {
        let subdir_path = target_path.join(subdir.path());
        extract_dir(subdir, &subdir_path)?;
    }

    Ok(())
}

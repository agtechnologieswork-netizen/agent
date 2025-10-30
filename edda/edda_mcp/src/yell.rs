use chrono::Utc;
use dialoguer::Input;
use eyre::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use sha2::{Sha256, Digest};
use std::fs::{self, File};
use std::path::Path;
use std::time::SystemTime;
use tar::Builder;

use crate::paths;

#[derive(Serialize)]
struct Metadata {
    timestamp: String,
    os: String,
    arch: String,
    version: String,
    binary_checksum: String,
}

pub fn run_yell(message: Option<String>) -> Result<()> {
    let trajectory_path = paths::trajectory_path()?;
    let session_log_dir = paths::session_log_dir();
    let output_dir = std::env::temp_dir();

    run_yell_with_paths(message, &trajectory_path, &session_log_dir, &output_dir)
}

pub fn run_yell_with_paths(
    message: Option<String>,
    trajectory_path: &Path,
    session_log_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
    // get bug description
    let description = match message {
        Some(msg) => msg,
        None => {
            Input::<String>::new()
                .with_prompt("Describe the bug (optional)")
                .interact_text()
                .wrap_err("failed to read input")?
        }
    };

    // create temp directory for bundle
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let bundle_name = format!("bug-report-{}", timestamp);
    let temp_dir = output_dir.join(&bundle_name);
    fs::create_dir_all(&temp_dir).wrap_err("failed to create temp directory")?;

    // collect trajectories
    if trajectory_path.exists() {
        let dest = temp_dir.join("history.jsonl");
        fs::copy(trajectory_path, &dest).wrap_err("failed to copy trajectory file")?;
    } else {
        eprintln!("⚠️  Warning: trajectory file not found at {:?}", trajectory_path);
    }

    // collect session logs from last 12h
    let logs_dir = temp_dir.join("logs");
    fs::create_dir_all(&logs_dir).wrap_err("failed to create logs directory")?;

    if session_log_dir.exists() {
        let cutoff = SystemTime::now() - std::time::Duration::from_secs(12 * 60 * 60);

        for entry in fs::read_dir(session_log_dir).wrap_err("failed to read session log directory")? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.starts_with("session-") && n.ends_with(".log")) {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        if modified >= cutoff {
                            let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                            let dest = logs_dir.join(&file_name);
                            fs::copy(&path, &dest).wrap_err_with(|| format!("failed to copy log file: {:?}", path))?;
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("⚠️  Warning: session log directory not found at {:?}", session_log_dir);
    }

    // write description
    let description_path = temp_dir.join("description.txt");
    fs::write(&description_path, &description).wrap_err("failed to write description file")?;

    // compute binary checksum (SHA256)
    let binary_checksum = match std::env::current_exe() {
        Ok(exe_path) => {
            let exe_bytes = fs::read(&exe_path).wrap_err("failed to read binary")?;
            let mut hasher = Sha256::new();
            hasher.update(&exe_bytes);
            format!("{:x}", hasher.finalize())
        }
        Err(_) => "unknown".to_string(),
    };

    // create metadata
    let metadata = Metadata {
        timestamp: Utc::now().to_rfc3339(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        binary_checksum,
    };

    let metadata_path = temp_dir.join("metadata.json");
    let metadata_json = serde_json::to_string_pretty(&metadata).wrap_err("failed to serialize metadata")?;
    fs::write(&metadata_path, metadata_json).wrap_err("failed to write metadata file")?;

    // create tar.gz bundle
    let bundle_path = output_dir.join(format!("edda-bug-report-{}.tar.gz", timestamp));
    let tar_gz = File::create(&bundle_path).wrap_err("failed to create bundle file")?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);

    // add all files from temp directory
    tar.append_dir_all(&bundle_name, &temp_dir)
        .wrap_err("failed to add files to archive")?;

    tar.finish().wrap_err("failed to finalize archive")?;

    // cleanup temp directory
    fs::remove_dir_all(&temp_dir).wrap_err("failed to cleanup temp directory")?;

    println!("\n✅ Bug report created: {}", bundle_path.display());
    println!("📤 Please send this file to the devs via Slack or eng-appbuild@databricks.com\n");

    Ok(())
}

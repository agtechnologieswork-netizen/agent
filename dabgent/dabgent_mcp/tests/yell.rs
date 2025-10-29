use dabgent_mcp::yell::run_yell_with_paths;
use eyre::Result;
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};
use tar::Archive;
use tempfile::TempDir;

#[tokio::test]
async fn test_yell_creates_bundle_with_message() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;

    let trajectory_path = temp_dir.path().join("history.jsonl");
    let session_log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&session_log_dir)?;

    // create mock trajectory file
    let trajectory_content = r#"{"session_id":"test-session","timestamp":"2025-01-01T00:00:00Z","tool_name":"test_tool","arguments":null,"success":true,"result":null,"error":null}"#;
    fs::write(&trajectory_path, trajectory_content)?;

    // run yell with message
    run_yell_with_paths(
        Some("test bug report".to_string()),
        &trajectory_path,
        &session_log_dir,
        output_dir.path(),
    )?;

    // verify bundle was created
    let bundles: Vec<_> = fs::read_dir(output_dir.path())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map_or(false, |n| n.starts_with("dabgent-bug-report-") && n.ends_with(".tar.gz"))
        })
        .collect();

    assert_eq!(bundles.len(), 1, "expected exactly one bundle file");

    Ok(())
}

#[tokio::test]
async fn test_yell_includes_full_trajectory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;

    let trajectory_path = temp_dir.path().join("history.jsonl");
    let session_log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&session_log_dir)?;

    // create trajectory with multiple entries
    let trajectory_content = r#"{"session_id":"session-1","timestamp":"2025-01-01T00:00:00Z","tool_name":"tool_1","arguments":null,"success":true,"result":null,"error":null}
{"session_id":"session-2","timestamp":"2025-01-02T00:00:00Z","tool_name":"tool_2","arguments":null,"success":true,"result":null,"error":null}
{"session_id":"session-3","timestamp":"2025-01-03T00:00:00Z","tool_name":"tool_3","arguments":null,"success":false,"result":null,"error":"test error"}"#;
    fs::write(&trajectory_path, trajectory_content)?;

    run_yell_with_paths(
        Some("trajectory test".to_string()),
        &trajectory_path,
        &session_log_dir,
        output_dir.path(),
    )?;

    // extract and verify trajectory content
    let bundle_path = find_bundle(output_dir.path())?;
    let extracted = extract_bundle(&bundle_path)?;
    let history_content = fs::read_to_string(extracted.path().join("history.jsonl"))?;

    assert_eq!(history_content, trajectory_content);
    assert_eq!(history_content.lines().count(), 3, "expected 3 trajectory entries");

    Ok(())
}

#[tokio::test]
async fn test_yell_filters_logs_by_24h() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;

    let trajectory_path = temp_dir.path().join("history.jsonl");
    let session_log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&session_log_dir)?;

    fs::write(&trajectory_path, "")?;

    // create old log (25h old)
    let old_log = session_log_dir.join("session-old.log");
    fs::write(&old_log, "old log content")?;
    let old_time = SystemTime::now() - Duration::from_secs(25 * 60 * 60);
    filetime::set_file_mtime(&old_log, filetime::FileTime::from_system_time(old_time))?;

    // create recent log (1h old)
    let recent_log = session_log_dir.join("session-recent.log");
    fs::write(&recent_log, "recent log content")?;
    let recent_time = SystemTime::now() - Duration::from_secs(1 * 60 * 60);
    filetime::set_file_mtime(&recent_log, filetime::FileTime::from_system_time(recent_time))?;

    run_yell_with_paths(
        Some("log filter test".to_string()),
        &trajectory_path,
        &session_log_dir,
        output_dir.path(),
    )?;

    // extract and verify only recent log is included
    let bundle_path = find_bundle(output_dir.path())?;
    let extracted = extract_bundle(&bundle_path)?;
    let logs_dir = extracted.path().join("logs");

    assert!(logs_dir.join("session-recent.log").exists(), "recent log should be included");
    assert!(!logs_dir.join("session-old.log").exists(), "old log should be excluded");

    let recent_content = fs::read_to_string(logs_dir.join("session-recent.log"))?;
    assert_eq!(recent_content, "recent log content");

    Ok(())
}

#[tokio::test]
async fn test_yell_handles_missing_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;

    let trajectory_path = temp_dir.path().join("nonexistent-history.jsonl");
    let session_log_dir = temp_dir.path().join("nonexistent-logs");

    // should not fail even if files don't exist
    let result = run_yell_with_paths(
        Some("missing files test".to_string()),
        &trajectory_path,
        &session_log_dir,
        output_dir.path(),
    );

    assert!(result.is_ok(), "should handle missing files gracefully");

    // bundle should still be created with description and metadata
    let bundle_path = find_bundle(output_dir.path())?;
    let extracted = extract_bundle(&bundle_path)?;

    assert!(extracted.path().join("description.txt").exists());
    assert!(extracted.path().join("metadata.json").exists());

    Ok(())
}

#[tokio::test]
async fn test_yell_bundle_is_valid_tarball() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;

    let trajectory_path = temp_dir.path().join("history.jsonl");
    let session_log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&session_log_dir)?;

    fs::write(&trajectory_path, r#"{"test":"data"}"#)?;

    run_yell_with_paths(
        Some("tarball test".to_string()),
        &trajectory_path,
        &session_log_dir,
        output_dir.path(),
    )?;

    let bundle_path = find_bundle(output_dir.path())?;

    // verify it's a valid gzip file
    let file = File::open(&bundle_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // verify we can list entries
    let entries: Vec<_> = archive.entries()?.collect();
    assert!(!entries.is_empty(), "archive should contain entries");

    // extract and verify structure
    let extracted = extract_bundle(&bundle_path)?;
    assert!(extracted.path().join("description.txt").exists());
    assert!(extracted.path().join("metadata.json").exists());

    // verify metadata is valid JSON
    let metadata_content = fs::read_to_string(extracted.path().join("metadata.json"))?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_content)?;

    assert!(metadata.get("timestamp").is_some());
    assert!(metadata.get("os").is_some());
    assert!(metadata.get("arch").is_some());
    assert!(metadata.get("version").is_some());
    assert!(metadata.get("binary_checksum").is_some());

    Ok(())
}

#[tokio::test]
async fn test_yell_includes_binary_checksum() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let output_dir = TempDir::new()?;

    let trajectory_path = temp_dir.path().join("history.jsonl");
    let session_log_dir = temp_dir.path().join("logs");
    fs::create_dir_all(&session_log_dir)?;

    fs::write(&trajectory_path, r#"{"test":"data"}"#)?;

    run_yell_with_paths(
        Some("checksum test".to_string()),
        &trajectory_path,
        &session_log_dir,
        output_dir.path(),
    )?;

    // extract and verify checksum is included
    let bundle_path = find_bundle(output_dir.path())?;
    let extracted = extract_bundle(&bundle_path)?;

    let metadata_content = fs::read_to_string(extracted.path().join("metadata.json"))?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_content)?;

    let checksum = metadata.get("binary_checksum").unwrap().as_str().unwrap();
    assert!(!checksum.is_empty(), "checksum should not be empty");
    // SHA256 hashes are 64 hex chars or "unknown"
    assert!(checksum == "unknown" || checksum.len() == 64, "checksum should be SHA256 hash or 'unknown'");
    if checksum != "unknown" {
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()), "checksum should be hex");
    }

    Ok(())
}

// helper: find the bundle file in output directory
fn find_bundle(dir: &Path) -> Result<std::path::PathBuf> {
    let bundles: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map_or(false, |n| n.starts_with("dabgent-bug-report-") && n.ends_with(".tar.gz"))
        })
        .collect();

    Ok(bundles[0].path())
}

// helper: extract bundle to temp directory
fn extract_bundle(bundle_path: &Path) -> Result<TempDir> {
    let extract_dir = TempDir::new()?;
    let file = File::open(bundle_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive.unpack(extract_dir.path())?;

    // find the bug-report-* subdirectory
    let entries: Vec<_> = fs::read_dir(extract_dir.path())?
        .filter_map(|e| e.ok())
        .collect();

    // return temp dir pointing to the extracted bug-report directory
    if let Some(entry) = entries.first() {
        let inner_dir = entry.path();
        let final_dir = TempDir::new()?;

        // copy contents to new tempdir for cleaner interface
        for entry in fs::read_dir(&inner_dir)? {
            let entry = entry?;
            let dest = final_dir.path().join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir_all(&entry.path(), &dest)?;
            } else {
                fs::copy(&entry.path(), &dest)?;
            }
        }

        Ok(final_dir)
    } else {
        Ok(extract_dir)
    }
}

// helper: recursively copy directory
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(&entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

use eyre::Result;
use std::path::PathBuf;

pub const SESSION_LOG_DIR: &str = "/tmp/dabgent-mcp";
pub const DABGENT_DIR: &str = ".dabgent";
pub const HISTORY_FILE: &str = "history.jsonl";

/// get the trajectory history file path (~/.dabgent/history.jsonl)
pub fn trajectory_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| eyre::eyre!("failed to get home directory"))?;
    Ok(home.join(DABGENT_DIR).join(HISTORY_FILE))
}

/// get the session log directory path (/tmp/dabgent-mcp)
pub fn session_log_dir() -> PathBuf {
    PathBuf::from(SESSION_LOG_DIR)
}

use chrono::Utc;
use eyre::Result;
use rmcp::model::{CallToolRequestParam, CallToolResult, ServerInfo};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};
use serde::{Deserialize, Serialize};
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::paths;
use crate::providers::CombinedProvider;

#[derive(Debug, Serialize, Deserialize)]
pub struct TrajectoryEntry {
    pub session_id: String,
    pub timestamp: String,
    pub tool_name: String,
    pub arguments: Option<serde_json::Value>,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub struct TrajectoryTrackingProvider {
    inner: CombinedProvider,
    history_file: Mutex<std::fs::File>,
    session_id: String,
}

impl TrajectoryTrackingProvider {
    pub fn new(inner: CombinedProvider, session_id: String) -> Result<Self> {
        let history_path = paths::trajectory_path()?;
        Self::new_with_path(inner, session_id, history_path)
    }

    #[doc(hidden)]
    pub fn new_with_path(
        inner: CombinedProvider,
        session_id: String,
        history_path: PathBuf,
    ) -> Result<Self> {
        // ensure parent directory exists
        if let Some(parent) = history_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let history_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(history_path)?;

        Ok(Self {
            inner,
            history_file: Mutex::new(history_file),
            session_id,
        })
    }

    fn record_trajectory(&self, entry: TrajectoryEntry) -> Result<()> {
        let json_line = serde_json::to_string(&entry)?;
        let mut file = self.history_file.lock().unwrap();
        writeln!(file, "{}", json_line)?;
        file.flush()?;
        Ok(())
    }
}

impl ServerHandler for TrajectoryTrackingProvider {
    fn get_info(&self) -> ServerInfo {
        self.inner.get_info()
    }

    async fn list_tools(
        &self,
        request: Option<rmcp::model::PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, ErrorData> {
        self.inner.list_tools(request, context).await
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let timestamp = Utc::now().to_rfc3339();
        let tool_name = params.name.to_string();
        let arguments = params.arguments.as_ref().map(|args| {
            serde_json::to_value(args).unwrap_or(serde_json::Value::Null)
        });

        // call inner provider
        let result = self.inner.call_tool(params, ctx).await;

        // record trajectory
        let entry = match &result {
            Ok(call_result) => TrajectoryEntry {
                session_id: self.session_id.clone(),
                timestamp,
                tool_name,
                arguments,
                success: !call_result.is_error.unwrap_or(false),
                result: Some(serde_json::to_value(call_result).unwrap_or(serde_json::Value::Null)),
                error: None,
            },
            Err(error_data) => TrajectoryEntry {
                session_id: self.session_id.clone(),
                timestamp,
                tool_name,
                arguments,
                success: false,
                result: None,
                error: Some(error_data.to_string()),
            },
        };

        if let Err(e) = self.record_trajectory(entry) {
            tracing::warn!("Failed to record trajectory: {}", e);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_entry_serialization_success() {
        let entry = TrajectoryEntry {
            session_id: "test-sess".to_string(),
            timestamp: "2025-10-29T10:00:00Z".to_string(),
            tool_name: "test_tool".to_string(),
            arguments: Some(serde_json::json!({"key": "value"})),
            success: true,
            result: Some(serde_json::json!({"output": "success"})),
            error: None,
        };

        let json_line = serde_json::to_string(&entry).unwrap();
        let deserialized: TrajectoryEntry = serde_json::from_str(&json_line).unwrap();

        assert_eq!(deserialized.session_id, "test-sess");
        assert_eq!(deserialized.tool_name, "test_tool");
        assert!(deserialized.success);
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn test_trajectory_entry_serialization_error() {
        let entry = TrajectoryEntry {
            session_id: "test-sess".to_string(),
            timestamp: "2025-10-29T10:00:00Z".to_string(),
            tool_name: "failing_tool".to_string(),
            arguments: None,
            success: false,
            result: None,
            error: Some("Tool execution failed".to_string()),
        };

        let json_line = serde_json::to_string(&entry).unwrap();
        let deserialized: TrajectoryEntry = serde_json::from_str(&json_line).unwrap();

        assert_eq!(deserialized.session_id, "test-sess");
        assert!(!deserialized.success);
        assert!(deserialized.result.is_none());
        assert_eq!(deserialized.error.unwrap(), "Tool execution failed");
    }

    #[test]
    fn test_trajectory_entry_jsonl_format() {
        let entry = TrajectoryEntry {
            session_id: "abc123".to_string(),
            timestamp: "2025-10-29T12:34:56Z".to_string(),
            tool_name: "deploy_app".to_string(),
            arguments: Some(serde_json::json!({"name": "myapp"})),
            success: true,
            result: Some(serde_json::json!({"url": "https://example.com"})),
            error: None,
        };

        let json_line = serde_json::to_string(&entry).unwrap();

        // should not contain newlines (JSONL requirement)
        assert!(!json_line.contains('\n'));

        // should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_line).unwrap();
        assert!(parsed.is_object());

        // should have all required fields
        let obj = parsed.as_object().unwrap();
        assert!(obj.contains_key("session_id"));
        assert!(obj.contains_key("timestamp"));
        assert!(obj.contains_key("tool_name"));
        assert!(obj.contains_key("success"));
    }
}

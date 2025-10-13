use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData;
use serde::Serialize;

/// Helper to wrap any serializable result into a CallToolResult
pub fn wrap_result<T: Serialize, E: std::fmt::Display>(
    result: Result<T, E>,
) -> Result<CallToolResult, ErrorData> {
    match result {
        Ok(data) => {
            let json = serde_json::to_string_pretty(&data)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(json)]))
        }
        Err(e) => Err(ErrorData::internal_error(e.to_string(), None)),
    }
}

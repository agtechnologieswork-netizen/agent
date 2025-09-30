use crate::toolbox::{ToolCallExt, ToolDyn};
use dabgent_sandbox::SandboxDyn;
use eyre::{OptionExt, Result};
use rig::message::{ToolCall, ToolResult};

pub async fn run_tools(
    sandbox: &mut Box<dyn SandboxDyn>,
    tools: &[Box<dyn ToolDyn>],
    calls: &[ToolCall],
) -> Result<Vec<ToolResult>> {
    let mut results = Vec::new();
    for call in calls {
        let tool = tools
            .iter()
            .find(|t| t.name() == call.function.name)
            .ok_or_eyre(format!("tool not found"))?;
        let result = tool.call(call.function.arguments.clone(), sandbox).await?;
        results.push(call.to_result(result));
    }
    Ok(results)
}

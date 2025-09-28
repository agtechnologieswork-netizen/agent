use super::{DelegationHandler, FinishDelegationTool};
use async_trait::async_trait;
use crate::event::{Event, ParentAggregate};
use crate::toolbox::ToolDyn;
use dabgent_sandbox::{SandboxDyn, NoOpSandbox, Sandbox};
use eyre::Result;
use uuid::Uuid;

const COMPACTION_SYSTEM_PROMPT: &str = r#"
You are an error message compactor. Your role is to reduce verbose error messages while preserving critical debugging information.

## Objectives
- Reduce error messages to the specified character limit while maintaining clarity
- Preserve essential information: error types, file paths, line numbers, root causes
- Remove unnecessary elements: repetitive stack traces, verbose details, redundant information

## Output Format
When you have compacted the error message, call the `finish_delegation` tool with your compacted result.

## Examples

### Example 1: Python Traceback
Input: A 800-character Python traceback with multiple stack frames
```
Traceback (most recent call last):
  File "/app/main.py", line 15, in <module>
    result = process_data()
  File "/app/main.py", line 10, in process_data
    return data.split(',')
AttributeError: 'NoneType' object has no attribute 'split'
[... additional verbose stack frames ...]
```
Output: `finish_delegation(result="AttributeError in main.py:10 - 'NoneType' object has no attribute 'split'")`

### Example 2: Validation Errors
Input: Verbose validation error with nested field details
```
ValidationError: Multiple validation errors occurred:
- Field 'name': This field is required and cannot be empty
- Field 'age': Value must be greater than or equal to 0
- Field 'email': Invalid email format, must contain @ symbol
[... additional validation context ...]
```
Output: `finish_delegation(result="ValidationError: 3 fields failed - name (required), age (min:0), email (invalid format)")`

### Example 3: Build/Compilation Errors
Input: Long compilation error with multiple issues
```
Error: Failed to compile TypeScript
src/components/UserForm.tsx(42,15): error TS2322: Type 'string' is not assignable to type 'number'
src/components/UserForm.tsx(45,8): error TS2304: Cannot find name 'useState'
[... additional compilation context ...]
```
Output: `finish_delegation(result="TypeScript errors: UserForm.tsx:42 (string→number), :45 (useState undefined)")`

## Instructions
Focus on the core issue and location. Remove implementation details that don't help identify the root cause.
Always call `finish_delegation` with your compacted result when done.
"#;

pub const TRIGGER_TOOL: &str = "compact_error";
pub const THREAD_PREFIX: &str = "compact_";
pub const WORKER_NAME: &str = "compact_worker";

pub struct CompactionHandler {
    sandbox: Box<dyn SandboxDyn>,
    tools: Vec<Box<dyn ToolDyn>>,
    compaction_threshold: usize,
}

impl CompactionHandler {
    pub fn new(compaction_threshold: usize) -> Result<Self> {
        let tools = vec![
            Box::new(FinishDelegationTool) as Box<dyn ToolDyn>
        ];

        Ok(Self {
            sandbox: NoOpSandbox::new().boxed(),
            tools,
            compaction_threshold,
        })
    }

    pub fn compaction_threshold(&self) -> usize {
        self.compaction_threshold
    }
}

#[async_trait]
impl DelegationHandler for CompactionHandler {
    fn trigger_tool(&self) -> &str {
        TRIGGER_TOOL
    }

    fn thread_prefix(&self) -> &str {
        THREAD_PREFIX
    }

    fn worker_name(&self) -> &str {
        WORKER_NAME
    }

    fn tools(&self) -> &[Box<dyn ToolDyn>] {
        &self.tools
    }

    async fn execute_tool_by_name(
        &mut self,
        tool_name: &str,
        args: serde_json::Value
    ) -> eyre::Result<Result<serde_json::Value, serde_json::Value>> {
        let tool = self.tools
            .iter()
            .find(|t| t.name() == tool_name)
            .ok_or_else(|| eyre::eyre!("Tool '{}' not found", tool_name))?;

        tool.call(args, &mut self.sandbox).await
    }

    fn handle(
        &self,
        _catalog: &str,  // Not used for compaction
        error_text: &str,
        model: &str,
        parent_aggregate_id: &str,
        parent_tool_id: &str
    ) -> Result<(String, Event, Event)> {
        let task_thread_id = format!("compact_{}", Uuid::new_v4());
        let prompt = format!("Compact this error message to under {} characters:\n\n{}",
                           self.compaction_threshold, error_text);

        let tool_definitions: Vec<rig::completion::ToolDefinition> = self.tools
            .iter()
            .map(|tool| tool.definition())
            .collect();

        let config_event = Event::LLMConfig {
            model: model.to_string(),
            temperature: 0.0,
            max_tokens: 8192,
            preamble: Some(COMPACTION_SYSTEM_PROMPT.to_string()),
            tools: Some(tool_definitions),
            recipient: Some(WORKER_NAME.to_string()),
            parent: Some(ParentAggregate {
                aggregate_id: parent_aggregate_id.to_string(),
                tool_id: Some(parent_tool_id.to_string()),
            }),
        };

        let user_event = Event::UserMessage(rig::OneOrMany::one(
            rig::message::UserContent::Text(rig::message::Text {
                text: prompt,
            }),
        ));

        Ok((task_thread_id, config_event, user_event))
    }

    fn format_result(&self, summary: &str) -> String {
        summary.to_string()
    }
}


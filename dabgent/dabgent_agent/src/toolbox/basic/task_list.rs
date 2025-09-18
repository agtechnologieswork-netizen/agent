use crate::toolbox::Tool;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

pub trait TaskList: Send + Sync {
    fn update(&self, current_content: String, instruction: String) -> Result<String>;
}

pub struct TaskListTool {
    sandbox: Arc<Mutex<Box<dyn SandboxDyn>>>,
    updater: Box<dyn TaskList>,
}

impl TaskListTool {
    pub fn new(sandbox: Box<dyn SandboxDyn>, updater: Box<dyn TaskList>) -> Self {
        Self {
            sandbox: Arc::new(Mutex::new(sandbox)),
            updater,
        }
    }

    pub fn with_updater<T: TaskList + 'static>(updater: T, sandbox: Box<dyn SandboxDyn>) -> Self {
        Self {
            sandbox: Arc::new(Mutex::new(sandbox)),
            updater: Box::new(updater),
        }
    }

    async fn execute_update(&self, instruction: String) -> Result<String> {
        let mut sandbox = self.sandbox.lock().await;

        let current_content = sandbox
            .read_file("planning.md")
            .await
            .unwrap_or_else(|_| "# Planning\n\nNo tasks yet.\n".to_string());

        let updated_content = self.updater.update(current_content, instruction.clone())?;

        sandbox.write_file("planning.md", &updated_content).await?;

        Ok(format!("Task list updated: {}", instruction))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListArgs {
    pub instruction: String,
}

impl Clone for TaskListTool {
    fn clone(&self) -> Self {
        Self {
            sandbox: self.sandbox.clone(),
            updater: Box::new(DefaultTaskList),
        }
    }
}

impl Tool for TaskListTool {
    type Args = TaskListArgs;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "update_task_list".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Update the planning.md task list file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "instruction": {
                        "type": "string",
                        "description": "Instructions for updating the task list",
                    }
                },
                "required": ["instruction"],
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
        _sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        match self.execute_update(args.instruction).await {
            Ok(msg) => Ok(Ok(msg)),
            Err(e) => Ok(Err(format!("Failed to update task list: {}", e))),
        }
    }
}

struct DefaultTaskList;

impl TaskList for DefaultTaskList {
    fn update(&self, current_content: String, instruction: String) -> Result<String> {
        Ok(format!("{}\n- {}", current_content, instruction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};

    struct MockTaskList {
        update_fn: StdArc<StdMutex<dyn FnMut(String) -> String + Send>>,
    }

    impl MockTaskList {
        fn new<F>(f: F) -> Self
        where
            F: FnMut(String) -> String + Send + 'static,
        {
            Self {
                update_fn: StdArc::new(StdMutex::new(f)),
            }
        }
    }

    impl TaskList for MockTaskList {
        fn update(&self, current_content: String, _instruction: String) -> Result<String> {
            let mut f = self.update_fn.lock().unwrap();
            Ok(f(current_content))
        }
    }

    #[tokio::test]
    async fn test_tasklist_tool() {
        use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};

        let opts = ConnectOpts::default();
        let result = opts.connect(|client| async move {
            let container = client
                .container()
                .from("alpine:latest")
                .with_exec(vec!["sh", "-c", "echo 'test environment ready'"]);

            container.sync().await?;
            let mut sandbox: Box<dyn SandboxDyn> = Box::new(DaggerSandbox::from_container(container, client.clone()));

            // Write initial file
            sandbox.write_file("planning.md", "# Planning\n\nNo tasks yet.\n").await.unwrap();

            let mock_tasklist = MockTaskList::new(|content| {
                format!("{}\n- Task completed", content)
            });

            // Create another sandbox for the tool
            let container2 = client
                .container()
                .from("alpine:latest")
                .with_exec(vec!["sh", "-c", "echo 'test environment ready'"]);
            container2.sync().await?;
            let sandbox2: Box<dyn SandboxDyn> = Box::new(DaggerSandbox::from_container(container2, client.clone()));

            let tool = TaskListTool::with_updater(mock_tasklist, sandbox2);

            let args = TaskListArgs {
                instruction: "Mark first task as complete".to_string(),
            };

            let result = tool.call(args, &mut sandbox).await.unwrap();
            assert!(result.is_ok());

            Ok::<(), eyre::Error>(())
        }).await;

        if result.is_err() {
            eprintln!("Skipping test - Docker/Dagger not available");
        }
    }
}
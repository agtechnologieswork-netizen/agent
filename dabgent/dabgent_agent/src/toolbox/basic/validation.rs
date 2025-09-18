use crate::toolbox::{Tool, Validator, ValidatorDyn};
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use serde_json::Value;

pub struct DoneTool {
    validator: Box<dyn ValidatorDyn>,
}

impl DoneTool {
    pub fn new<T: Validator>(validator: T) -> Self {
        Self {
            validator: validator.boxed(),
        }
    }
}

impl Tool for DoneTool {
    type Args = Value;
    type Output = String;
    type Error = String;

    fn name(&self) -> String {
        "done".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Run checks, if successful mark task as finished".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
        sandbox: &mut Box<dyn SandboxDyn>,
    ) -> Result<Result<Self::Output, Self::Error>> {
        match self.validator.run(sandbox).await {
            Ok(Ok(_)) => Ok(Ok("success".to_string())),
            Ok(Err(err)) => Ok(Err(format!("validation error: {}", err))),
            Err(e) => Ok(Err(format!("validator failed: {}", e))),
        }
    }
}

pub struct TaskListValidator<V: Validator> {
    inner: V,
}

impl<V: Validator> TaskListValidator<V> {
    pub fn new(inner: V) -> Self {
        Self { inner }
    }

    async fn check_tasks(&self, content: &str) -> Result<(), String> {
        let has_incomplete = content.contains("[ ]");
        let has_completed = content.contains("[x]") || content.contains("[X]");

        if has_incomplete {
            Err("Not all tasks are completed".to_string())
        } else if !has_completed {
            Err("No completed tasks found".to_string())
        } else {
            Ok(())
        }
    }
}

impl<V: Validator> Validator for TaskListValidator<V> {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        if let Ok(content) = sandbox.read_file("planning.md").await {
            if let Err(e) = self.check_tasks(&content).await {
                return Ok(Err(e));
            }
        }
        self.inner.run(sandbox).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysPassValidator;
    impl Validator for AlwaysPassValidator {
        async fn run(&self, _sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
            Ok(Ok(()))
        }
    }

    #[tokio::test]
    async fn test_task_list_validator() {
        use dabgent_sandbox::dagger::{ConnectOpts, Sandbox as DaggerSandbox};

        let opts = ConnectOpts::default();
        let result = opts.connect(|client| async move {
            let container = client
                .container()
                .from("python:3.11-slim")
                .with_workdir("/workspace");

            container.sync().await?;
            let mut sandbox: Box<dyn SandboxDyn> = Box::new(DaggerSandbox::from_container(container, client.clone()));

            let validator = TaskListValidator::new(AlwaysPassValidator);

            // Test 1: No planning.md file - should pass
            let result = Validator::run(&validator, &mut sandbox).await?;
            assert!(result.is_ok());

            // Test 2: Incomplete tasks - should fail
            sandbox.write_file("planning.md", "# Tasks\n- [ ] Task 1\n- [x] Task 2").await?;
            let result = Validator::run(&validator, &mut sandbox).await?;
            assert!(result.is_err());

            // Test 3: All tasks completed - should pass
            sandbox.write_file("planning.md", "# Tasks\n- [x] Task 1\n- [x] Task 2").await?;
            let result = Validator::run(&validator, &mut sandbox).await?;
            assert!(result.is_ok());

            Ok::<(), eyre::Error>(())
        }).await;

        if result.is_err() {
            eprintln!("Skipping test - Docker/Dagger not available");
        }
    }
}
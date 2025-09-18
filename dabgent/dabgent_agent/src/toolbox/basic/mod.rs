
mod file_ops;
mod task_list;
mod validation;

pub use file_ops::*;
pub use task_list::{TaskList, TaskListTool};
pub use validation::{DoneTool, TaskListValidator};

use crate::toolbox::{Validator, ToolDyn};

pub fn toolset<T: Validator>(validator: T) -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(Bash),
        Box::new(WriteFile),
        Box::new(ReadFile),
        Box::new(LsDir),
        Box::new(RmFile),
        Box::new(EditFile),
        Box::new(DoneTool::new(validator)),
    ]
}

pub fn toolset_with_tasklist<V, T>(
    validator: V,
    task_list: T,
    sandbox: Box<dyn dabgent_sandbox::SandboxDyn>,
) -> Vec<Box<dyn ToolDyn>>
where
    V: Validator,
    T: TaskList + 'static,
{
    let task_list_validator = TaskListValidator::new(validator);

    vec![
        Box::new(Bash),
        Box::new(ReadFile),
        Box::new(EditFile),
        Box::new(TaskListTool::with_updater(task_list, sandbox)),
        Box::new(DoneTool::new(task_list_validator)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolset_creation() {
        struct DummyValidator;
        impl Validator for DummyValidator {
            async fn run(
                &self,
                _sandbox: &mut Box<dyn dabgent_sandbox::SandboxDyn>,
            ) -> eyre::Result<Result<(), String>> {
                Ok(Ok(()))
            }
        }

        let tools = toolset(DummyValidator);
        assert_eq!(tools.len(), 7);

        let names: Vec<String> = tools.iter().map(|t| t.name()).collect();
        assert!(names.contains(&"bash".to_string()));
        assert!(names.contains(&"write_file".to_string()));
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"done".to_string()));
    }
}
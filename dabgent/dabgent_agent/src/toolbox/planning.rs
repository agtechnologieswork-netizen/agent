use crate::planner::{Planner, ThreadSettings};
use dabgent_mq::EventStore;
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use super::{NoSandboxTool, NoSandboxAdapter};

/// Tool for creating an initial plan from a task description
pub struct CreatePlanTool<S: EventStore> {
    planner: Arc<Mutex<Option<Planner<S>>>>,
    store: S,
    stream_id: String,
    settings: ThreadSettings,
}

impl<S: EventStore + Clone> CreatePlanTool<S> {
    pub fn new(
        planner: Arc<Mutex<Option<Planner<S>>>>,
        store: S,
        stream_id: String,
        settings: ThreadSettings,
    ) -> Self {
        Self {
            planner,
            store,
            stream_id,
            settings,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePlanArgs {
    pub tasks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePlanOutput {
    pub tasks: Vec<String>,
    pub message: String,
}

impl<S: EventStore + Clone + Send + Sync> NoSandboxTool for CreatePlanTool<S> {
    type Args = CreatePlanArgs;
    type Output = CreatePlanOutput;
    type Error = String;

    fn name(&self) -> String {
        "create_plan".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Create a plan by breaking down a task into concrete, actionable steps.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "A concrete, actionable task description"
                        },
                        "description": "An ordered list of tasks to complete"
                    }
                },
                "required": ["tasks"]
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let mut planner_lock = match self.planner.lock() {
            Ok(lock) => lock,
            Err(_) => return Ok(Err("Failed to acquire planner lock".to_string())),
        };

        // Create new planner
        let mut planner = Planner::new(
            self.store.clone(),
            self.stream_id.clone(),
            self.settings.clone(),
        );

        // Set tasks from the structured input
        planner.set_tasks(args.tasks.clone());

        // Return the tasks
        let tasks = args.tasks;

        let message = format!("Created plan with {} tasks", tasks.len());

        // Store the planner
        *planner_lock = Some(planner);

        Ok(Ok(CreatePlanOutput { tasks, message }))
    }
}

/// Tool for updating an existing plan
pub struct UpdatePlanTool<S: EventStore> {
    planner: Arc<Mutex<Option<Planner<S>>>>,
}

impl<S: EventStore> UpdatePlanTool<S> {
    pub fn new(planner: Arc<Mutex<Option<Planner<S>>>>) -> Self {
        Self { planner }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePlanArgs {
    pub tasks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePlanOutput {
    pub tasks: Vec<String>,
    pub message: String,
}

impl<S: EventStore + Send + Sync> NoSandboxTool for UpdatePlanTool<S> {
    type Args = UpdatePlanArgs;
    type Output = UpdatePlanOutput;
    type Error = String;

    fn name(&self) -> String {
        "update_plan".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Update the existing plan with a new set of tasks".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "A concrete, actionable task description"
                        },
                        "description": "An updated ordered list of tasks to complete"
                    }
                },
                "required": ["tasks"]
            }),
        }
    }

    async fn call(
        &self,
        args: Self::Args,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let mut planner_lock = match self.planner.lock() {
            Ok(lock) => lock,
            Err(_) => return Ok(Err("Failed to acquire planner lock".to_string())),
        };

        let planner = match planner_lock.as_mut() {
            Some(p) => p,
            None => return Ok(Err("No plan exists yet. Use create_plan first.".to_string())),
        };

        // Update the plan with structured tasks
        planner.set_tasks(args.tasks.clone());

        // Return the updated tasks
        let tasks = args.tasks;

        let message = format!("Updated plan with {} tasks", tasks.len());

        Ok(Ok(UpdatePlanOutput { tasks, message }))
    }
}

/// Tool for getting the current plan status
pub struct GetPlanStatusTool<S: EventStore> {
    planner: Arc<Mutex<Option<Planner<S>>>>,
}

impl<S: EventStore> GetPlanStatusTool<S> {
    pub fn new(planner: Arc<Mutex<Option<Planner<S>>>>) -> Self {
        Self { planner }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPlanStatusArgs {}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub description: String,
    pub thread_id: String,
    pub completed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPlanStatusOutput {
    pub tasks: Vec<TaskStatus>,
    pub completed_count: usize,
    pub total_count: usize,
}

impl<S: EventStore + Send + Sync> NoSandboxTool for GetPlanStatusTool<S> {
    type Args = GetPlanStatusArgs;
    type Output = GetPlanStatusOutput;
    type Error = String;

    fn name(&self) -> String {
        "get_plan_status".to_string()
    }

    fn definition(&self) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: self.name(),
            description: "Get the current status of all tasks in the plan".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn call(
        &self,
        _args: Self::Args,
    ) -> Result<Result<Self::Output, Self::Error>> {
        let planner_lock = match self.planner.lock() {
            Ok(lock) => lock,
            Err(_) => return Ok(Err("Failed to acquire planner lock".to_string())),
        };

        let planner = match planner_lock.as_ref() {
            Some(p) => p,
            None => return Ok(Err("No plan exists yet. Use create_plan first.".to_string())),
        };

        let tasks: Vec<TaskStatus> = planner
            .tasks()
            .iter()
            .map(|t| TaskStatus {
                description: t.description.clone(),
                thread_id: t.thread().to_string(),
                completed: t.completed,
            })
            .collect();

        let completed_count = tasks.iter().filter(|t| t.completed).count();
        let total_count = tasks.len();

        Ok(Ok(GetPlanStatusOutput {
            tasks,
            completed_count,
            total_count,
        }))
    }
}

pub fn planning_toolset<S: EventStore + Clone + Send + Sync + 'static>(
    planner: Arc<Mutex<Option<Planner<S>>>>,
    store: S,
    stream_id: String,
    settings: ThreadSettings,
) -> Vec<Box<dyn super::ToolDyn>> {
    vec![
        Box::new(NoSandboxAdapter::new(CreatePlanTool::new(planner.clone(), store, stream_id, settings))),
        Box::new(NoSandboxAdapter::new(UpdatePlanTool::new(planner.clone()))),
        Box::new(NoSandboxAdapter::new(GetPlanStatusTool::new(planner))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{Planner, ThreadSettings};
    use dabgent_mq::db::sqlite::SqliteStore;
    use sqlx::SqlitePool;

    async fn test_store() -> SqliteStore {
        let pool = SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");
        let store = SqliteStore::new(pool);
        store.migrate().await;
        store
    }

    #[tokio::test]
    async fn test_create_plan_tool() {
        let store = test_store().await;
        let planner = Arc::new(Mutex::new(None));
        let settings = ThreadSettings::new("test-model", 0.7, 1024);

        let tool = CreatePlanTool::new(
            planner.clone(),
            store.clone(),
            "test-stream".to_string(),
            settings,
        );

        // Test tool metadata
        assert_eq!(tool.name(), "create_plan");
        assert!(tool.definition().description.contains("Create a plan"));

        // Create a plan with structured tasks
        let args = CreatePlanArgs {
            tasks: vec!["Task 1".to_string(), "Task 2".to_string(), "Task 3".to_string()],
        };

        let result = tool.call(args).await.unwrap().unwrap();
        assert_eq!(result.tasks.len(), 3);
        assert_eq!(result.tasks[0], "Task 1");
        assert_eq!(result.tasks[1], "Task 2");
        assert_eq!(result.tasks[2], "Task 3");
        assert!(result.message.contains("3 tasks"));

        // Verify planner was stored
        assert!(planner.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn test_update_plan_tool() {
        let store = test_store().await;
        let settings = ThreadSettings::new("test-model", 0.7, 1024);

        // First create a planner with initial tasks
        let planner = Arc::new(Mutex::new(None));
        let mut initial_planner = Planner::new(
            store.clone(),
            "test-stream".to_string(),
            settings.clone(),
        );
        initial_planner.set_tasks(vec!["Initial task".to_string()]);
        *planner.lock().unwrap() = Some(initial_planner);

        // Now test updating the plan
        let tool = UpdatePlanTool::new(planner.clone());

        assert_eq!(tool.name(), "update_plan");

        let args = UpdatePlanArgs {
            tasks: vec!["Updated task 1".to_string(), "Updated task 2".to_string()],
        };

        let result = tool.call(args).await.unwrap().unwrap();
        assert_eq!(result.tasks.len(), 2);
        assert_eq!(result.tasks[0], "Updated task 1");
        assert_eq!(result.tasks[1], "Updated task 2");
        assert!(result.message.contains("2 tasks"));
    }

    #[tokio::test]
    async fn test_update_plan_without_existing_plan() {
        let _store = test_store().await;
        let planner: Arc<Mutex<Option<Planner<SqliteStore>>>> = Arc::new(Mutex::new(None));
        let tool = UpdatePlanTool::new(planner);

        let args = UpdatePlanArgs {
            tasks: vec!["Task".to_string()],
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.contains("No plan exists"));
            assert!(error.contains("create_plan first"));
        }
    }

    #[tokio::test]
    async fn test_get_plan_status_tool() {
        let store = test_store().await;
        let settings = ThreadSettings::new("test-model", 0.7, 1024);

        // Create a planner with some tasks using CreatePlanTool
        let planner = Arc::new(Mutex::new(None));

        let create_tool = CreatePlanTool::new(
            planner.clone(),
            store.clone(),
            "test-stream".to_string(),
            settings.clone(),
        );

        let create_args = CreatePlanArgs {
            tasks: vec!["Task A".to_string(), "Task B".to_string(), "Task C".to_string()],
        };

        create_tool.call(create_args).await.unwrap().unwrap();

        let tool = GetPlanStatusTool::new(planner.clone());
        assert_eq!(tool.name(), "get_plan_status");

        let args = GetPlanStatusArgs {};
        let result = tool.call(args).await.unwrap().unwrap();

        assert_eq!(result.total_count, 3);
        assert_eq!(result.completed_count, 0); // No tasks completed yet
        assert_eq!(result.tasks.len(), 3);

        assert_eq!(result.tasks[0].description, "Task A");
        assert!(!result.tasks[0].completed);
        assert_eq!(result.tasks[0].thread_id, "task-0");

        assert_eq!(result.tasks[1].description, "Task B");
        assert!(!result.tasks[1].completed);
        assert_eq!(result.tasks[1].thread_id, "task-1");

        assert_eq!(result.tasks[2].description, "Task C");
        assert!(!result.tasks[2].completed);
        assert_eq!(result.tasks[2].thread_id, "task-2");
    }

    #[tokio::test]
    async fn test_get_plan_status_without_plan() {
        let _store = test_store().await;
        let planner: Arc<Mutex<Option<Planner<SqliteStore>>>> = Arc::new(Mutex::new(None));
        let tool = GetPlanStatusTool::new(planner);

        let args = GetPlanStatusArgs {};
        let result = tool.call(args).await.unwrap();

        assert!(result.is_err());
        if let Err(error) = result {
            assert!(error.contains("No plan exists"));
            assert!(error.contains("create_plan first"));
        }
    }

    #[tokio::test]
    async fn test_planning_toolset_integration() {
        let store = test_store().await;
        let settings = ThreadSettings::new("test-model", 0.7, 1024);
        let planner = Arc::new(Mutex::new(None));

        let tools = planning_toolset(
            planner.clone(),
            store.clone(),
            "test-stream".to_string(),
            settings,
        );

        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name(), "create_plan");
        assert_eq!(tools[1].name(), "update_plan");
        assert_eq!(tools[2].name(), "get_plan_status");
    }

    #[tokio::test]
    async fn test_create_plan_with_empty_input() {
        let store = test_store().await;
        let planner = Arc::new(Mutex::new(None));
        let settings = ThreadSettings::new("test-model", 0.7, 1024);

        let tool = CreatePlanTool::new(
            planner.clone(),
            store.clone(),
            "test-stream".to_string(),
            settings,
        );

        let args = CreatePlanArgs {
            tasks: vec![],
        };

        let result = tool.call(args).await.unwrap().unwrap();
        assert_eq!(result.tasks.len(), 0);
        assert!(result.message.contains("0 tasks"));
    }

    #[tokio::test]
    async fn test_create_plan_with_multiple_tasks() {
        let store = test_store().await;
        let planner = Arc::new(Mutex::new(None));
        let settings = ThreadSettings::new("test-model", 0.7, 1024);

        let tool = CreatePlanTool::new(
            planner.clone(),
            store.clone(),
            "test-stream".to_string(),
            settings,
        );

        // Test with multiple tasks
        let args = CreatePlanArgs {
            tasks: vec![
                "Create main file".to_string(),
                "Add necessary imports".to_string(),
                "Implement core functionality".to_string(),
                "Add error handling".to_string(),
                "Test the implementation".to_string(),
            ],
        };

        let result = tool.call(args).await.unwrap().unwrap();
        assert_eq!(result.tasks.len(), 5);
        assert_eq!(result.tasks[0], "Create main file");
        assert_eq!(result.tasks[1], "Add necessary imports");
        assert_eq!(result.tasks[2], "Implement core functionality");
        assert_eq!(result.tasks[3], "Add error handling");
        assert_eq!(result.tasks[4], "Test the implementation");
    }
}
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
    pub plan: String,
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
            description: "Create a plan by breaking down a task into steps. Provide the plan as a bulleted list.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "plan": {
                        "type": "string",
                        "description": "The plan as a bulleted list with each task on a new line starting with - or *"
                    }
                },
                "required": ["plan"]
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

        // Parse the plan
        planner.plan(&args.plan);

        // Get task descriptions
        let tasks: Vec<String> = planner
            .tasks()
            .iter()
            .map(|t| t.description.clone())
            .collect();

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
    pub updated_plan: String,
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
                    "updated_plan": {
                        "type": "string",
                        "description": "The updated plan as a bulleted list"
                    }
                },
                "required": ["updated_plan"]
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

        // Update the plan
        planner.plan(&args.updated_plan);

        // Get updated task descriptions
        let tasks: Vec<String> = planner
            .tasks()
            .iter()
            .map(|t| t.description.clone())
            .collect();

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
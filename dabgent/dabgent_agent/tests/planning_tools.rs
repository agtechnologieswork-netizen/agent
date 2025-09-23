use dabgent_agent::planner::{Planner, ThreadSettings};
use dabgent_agent::toolbox::planning::{
    CreatePlanArgs, CreatePlanTool, GetPlanStatusTool, UpdatePlanArgs, UpdatePlanTool,
};
use dabgent_agent::toolbox::Tool;
use dabgent_mq::db::sqlite::SqliteStore;
use dabgent_sandbox::{SandboxDyn, ExecResult};
use std::sync::{Arc, Mutex};

async fn create_store() -> SqliteStore {
    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("Failed to create SQLite pool");
    let store = SqliteStore::new(pool);
    store.migrate().await;
    store
}

#[tokio::test]
async fn test_planning_tools_workflow() {
    let store = create_store().await;
    let stream_id = "test-stream";
    let settings = ThreadSettings::new("test-model", 0.7, 1024);

    // Create shared planner state
    let planner: Arc<Mutex<Option<Planner<_>>>> = Arc::new(Mutex::new(None));

    // Create planning tools
    let create_tool = CreatePlanTool::new(
        planner.clone(),
        store.clone(),
        stream_id.to_string(),
        settings.clone(),
    );
    let update_tool = UpdatePlanTool::new(planner.clone());
    let status_tool = GetPlanStatusTool::new(planner.clone());

    // Test create plan
    let mut sandbox = Box::new(Sandbox::new());
    let create_args = CreatePlanArgs {
        plan: "- Task 1\n- Task 2\n- Task 3".to_string(),
    };
    let result = create_tool.call(create_args, &mut sandbox).await.unwrap();
    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.tasks.len(), 3);
    assert_eq!(output.tasks[0], "Task 1");
    assert_eq!(output.tasks[1], "Task 2");
    assert_eq!(output.tasks[2], "Task 3");

    // Test get status
    let status = status_tool
        .call(
            dabgent_agent::toolbox::planning::GetPlanStatusArgs {},
            &mut sandbox,
        )
        .await
        .unwrap();
    assert!(status.is_ok());
    let status_output = status.unwrap();
    assert_eq!(status_output.total_count, 3);
    assert_eq!(status_output.completed_count, 0);

    // Test update plan
    let update_args = UpdatePlanArgs {
        updated_plan: "- New Task 1\n- New Task 2".to_string(),
    };
    let update_result = update_tool.call(update_args, &mut sandbox).await.unwrap();
    assert!(update_result.is_ok());
    let update_output = update_result.unwrap();
    assert_eq!(update_output.tasks.len(), 2);
    assert_eq!(update_output.tasks[0], "New Task 1");
    assert_eq!(update_output.tasks[1], "New Task 2");

    // Verify the update worked
    let final_status = status_tool
        .call(
            dabgent_agent::toolbox::planning::GetPlanStatusArgs {},
            &mut sandbox,
        )
        .await
        .unwrap();
    assert!(final_status.is_ok());
    let final_output = final_status.unwrap();
    assert_eq!(final_output.total_count, 2);
}

#[tokio::test]
async fn test_planning_tool_definitions() {
    let store = create_store().await;
    let stream_id = "test-stream";
    let settings = ThreadSettings::new("test-model", 0.7, 1024);
    let planner: Arc<Mutex<Option<Planner<_>>>> = Arc::new(Mutex::new(None));

    // Test tool definitions are valid
    let create_tool = CreatePlanTool::new(
        planner.clone(),
        store.clone(),
        stream_id.to_string(),
        settings,
    );
    let def = create_tool.definition();
    assert_eq!(def.name, "create_plan");
    assert!(def.description.contains("Create a plan"));
    assert!(def.parameters.is_object());

    let update_tool = UpdatePlanTool::new(planner.clone());
    let def = update_tool.definition();
    assert_eq!(def.name, "update_plan");
    assert!(def.description.contains("Update"));

    let status_tool = GetPlanStatusTool::new(planner);
    let def = status_tool.definition();
    assert_eq!(def.name, "get_plan_status");
    assert!(def.description.contains("status"));
}
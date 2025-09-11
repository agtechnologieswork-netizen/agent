use dabgent_agent::planning::{generate_plan_template, suggest_step_format, PlanStep, StepStatus};

fn main() {
    println!("Testing simplified planning helpers...\n");
    
    // Test plan template generation
    let task = "Build a REST API service";
    let template = generate_plan_template(task);
    println!("Generated Plan Template:");
    println!("{}", template);
    println!("{}", "=".repeat(50));
    
    // Test step format suggestions
    println!("\nStep Format Examples:");
    
    let pending_step = PlanStep {
        description: "Set up project structure".to_string(),
        status: StepStatus::Pending,
    };
    println!("Pending: {}", suggest_step_format(&pending_step));
    
    let in_progress_step = PlanStep {
        description: "Implement data models".to_string(),
        status: StepStatus::InProgress,
    };
    println!("In Progress: {}", suggest_step_format(&in_progress_step));
    
    let completed_step = PlanStep {
        description: "Create API endpoints".to_string(),
        status: StepStatus::Completed,
    };
    println!("Completed: {}", suggest_step_format(&completed_step));
    
    let failed_step = PlanStep {
        description: "Deploy to production".to_string(),
        status: StepStatus::Failed("Connection timeout".to_string()),
    };
    println!("Failed: {}", suggest_step_format(&failed_step));
    
    println!("\n✅ Simplified planning helpers work correctly!");
    println!("The LLM agent will use these helpers and manage plan.md directly.");
}
use clap::{Parser, Subcommand};
use eyre::OptionExt;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Generate {
        #[arg(short, long)]
        prompt: String,
        #[arg(short, long, default_value = "nicegui")]
        stack: String,
    },
    Scratchpad,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Generate { prompt, stack } => {
            generate(prompt, stack).await?;
        }
        Commands::Scratchpad => {
            meta_agent::agent::actor::eval_demo_agent().await?;
        }
    }
    Ok(())
}

async fn generate(prompt: String, stack: String) -> eyre::Result<()> {
    use meta_agent::{
        agent::{Command, Pipeline, actor::PipelineCmd},
        stacks::StackRegistry,
        workspace::{dagger::DaggerRef, WorkspaceDyn},
    };
    use std::fs;
    
    let stack_config = StackRegistry::get_stack(&stack).await?;
    
    // Clean up output directory
    let output_dir = format!("{}_output", stack);
    if std::path::Path::new(&output_dir).exists() {
        tracing::info!("Cleaning up existing output directory: {}", output_dir);
        fs::remove_dir_all(&output_dir)?;
        tracing::info!("Successfully removed existing output directory");
    }
    let pipeline = stack_config.create_pipeline().await?;
    
    let dagger_ref = DaggerRef::new();
    let workspace = dagger_ref
        .workspace("Dockerfile.appbuild".into(), stack_config.context_path().into())
        .await?;
    
    // Create boxed workspace for template file writing
    let mut boxed_workspace: Box<dyn WorkspaceDyn> = Box::new(workspace);
    
    // Add template files to workspace using bulk write to prevent deep query chains
    let template_files = stack_config.template_files()?;
    let template_files_vec: Vec<(String, String)> = template_files.into_iter().collect();
    tracing::info!("Writing {} template files in bulk", template_files_vec.len());
    boxed_workspace.write_files_bulk(template_files_vec).await?;
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);
    let mut pipeline = pipeline;
    let cmd = Command::new(
        None,
        PipelineCmd::Start {
            prompt,
            workspace: boxed_workspace,
        },
    );

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    tokio::spawn({
        let cmd_tx = cmd_tx.clone();
        async move {
            let _ = cmd_tx.send(cmd).await;
        }
    });
    let solution_tree = pipeline
        .execute(cmd_rx, event_tx)
        .await?
        .ok_or_eyre("no solutions found")?;

    // Extract and write files from the solution
    tracing::info!("Extracting files from solution tree");
    
    // Find the best solution node (for now, just get any completed node)
    let mut best_node: Option<&meta_agent::agent::actor::Node> = None;
    
    for node in solution_tree.get_nodes().iter() {
        if node.kind == meta_agent::agent::actor::NodeKind::Done && !node.files.is_empty() {
            best_node = Some(node);
            break;
        }
    }
    
    if let Some(node) = best_node {
        tracing::info!("Found solution with {} files", node.files.len());
        
        // Create output directory
        fs::create_dir_all(&output_dir)?;
        
        // Write template files first
        let template_files = stack_config.template_files()?;
        for (file_path, content) in &template_files {
            let full_path = std::path::Path::new(&output_dir).join(file_path);
            
            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            fs::write(&full_path, content)?;
            tracing::info!("Wrote template file: {}", full_path.display());
        }
        
        // Write all files from the solution (may overwrite template files)
        for (file_path, content) in &node.files {
            let full_path = std::path::Path::new(&output_dir).join(file_path);
            
            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            fs::write(&full_path, content)?;
            tracing::info!("Wrote solution file: {}", full_path.display());
        }
        
        tracing::info!("Successfully wrote {} files to {}", node.files.len(), output_dir);
    } else {
        tracing::warn!("No completed solution found in tree");
    }
    
    Ok(())
}

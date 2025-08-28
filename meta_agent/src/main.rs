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
        workspace::dagger::DaggerRef,
    };
    
    let stack_config = StackRegistry::get_stack(&stack).await?;
    let pipeline = stack_config.create_pipeline().await?;
    
    let dagger_ref = DaggerRef::new();
    let workspace = dagger_ref
        .workspace("Dockerfile.appbuild".into(), stack_config.context_path().into())
        .await?;
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);
    let mut pipeline = pipeline;
    let cmd = Command::new(
        None,
        PipelineCmd::Start {
            prompt: prompt,
            workspace: Box::new(workspace),
        },
    );

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    tokio::spawn({
        let cmd_tx = cmd_tx.clone();
        async move {
            let _ = cmd_tx.send(cmd).await;
        }
    });
    let _ = pipeline
        .execute(cmd_rx, event_tx)
        .await?
        .ok_or_eyre("no solutions found")?;
    Ok(())
}

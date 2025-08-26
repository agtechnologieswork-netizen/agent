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
    },
    Scratchpad,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Generate { prompt } => {
            generate(prompt).await?;
        }
        Commands::Scratchpad => {
            meta_agent::agent::actor::eval_demo_agent().await?;
        }
    }
    Ok(())
}

async fn generate(prompt: String) -> eyre::Result<()> {
    use meta_agent::{
        agent::actor::{self},
        agent::{Command, Pipeline},
        workspace::dagger::DaggerRef,
    };
    let dagger_ref = DaggerRef::new();
    let workspace = dagger_ref
        .workspace("Dockerfile.appbuild".into(), "./src/stacks/python".into())
        .await?;
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1);
    let mut pipeline = actor::claude_python_pipeline().await?;
    let cmd = Command::new(
        None,
        actor::PipelineCmd::Start {
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

use clap::Parser;
use dabgent_sandbox::dagger::{ConnectOpts, Logger};
use dabgent_screenshot::{screenshot_app, screenshot_apps_batch, ScreenshotOptions};
use eyre::Result;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "dabgent-screenshot")]
#[command(about = "Screenshot web applications using Playwright")]
#[command(version)]
struct Cli {
    /// Enable verbose Dagger logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser)]
enum Commands {
    /// Screenshot a single app from a Dockerfile directory
    App {
        /// Directory containing app source and Dockerfile
        #[arg(long)]
        app_source: String,

        /// Environment variables (KEY=VALUE,KEY2=VALUE2)
        #[arg(long)]
        env_vars: Option<String>,

        /// Port the app listens on
        #[arg(long, default_value = "8000")]
        port: u16,

        /// Wait time in milliseconds for network idle
        #[arg(long, default_value = "60000")]
        wait_time: u64,

        /// Output path for screenshots
        #[arg(long, default_value = "./screenshots")]
        output: String,
    },

    /// Screenshot multiple apps in batch
    Batch {
        /// Directories containing app sources (comma-separated)
        #[arg(long, value_delimiter = ',')]
        app_sources: Vec<String>,

        /// Environment variables shared across all apps (KEY=VALUE,KEY2=VALUE2)
        #[arg(long)]
        env_vars: Option<String>,

        /// Port all apps listen on
        #[arg(long, default_value = "8000")]
        port: u16,

        /// Wait time in milliseconds for network idle
        #[arg(long, default_value = "60000")]
        wait_time: u64,

        /// Number of apps to process in parallel
        #[arg(long, default_value = "3")]
        concurrency: usize,

        /// Output path for screenshots
        #[arg(long, default_value = "./screenshots")]
        output: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::App {
            app_source,
            env_vars,
            port,
            wait_time,
            output,
        } => {
            let env_vars = parse_env_vars(env_vars.as_deref().unwrap_or(""));

            let options = ScreenshotOptions {
                port,
                wait_time_ms: wait_time,
                env_vars,
                ..Default::default()
            };

            // connect to Dagger with options
            let logger = if cli.verbose {
                Logger::Default
            } else {
                Logger::Silent
            };
            let opts = ConnectOpts::default()
                .with_logger(logger)
                .with_execute_timeout(Some(600));

            opts.connect(move |client| async move {
                let app_source_dir = client.host().directory(&app_source);

                tracing::info!("Screenshotting app from: {}", app_source);

                let screenshots_dir = screenshot_app(&client, app_source_dir, options).await?;

                tracing::info!("Exporting screenshots to: {}", output);
                screenshots_dir.export(&output).await?;

                println!("✓ Screenshots saved to: {}", output);
                Ok(())
            })
            .await?;
        }

        Commands::Batch {
            app_sources,
            env_vars,
            port,
            wait_time,
            concurrency,
            output,
        } => {
            if app_sources.is_empty() {
                eprintln!("Error: No app sources provided");
                std::process::exit(1);
            }

            let env_vars = parse_env_vars(env_vars.as_deref().unwrap_or(""));

            let options = ScreenshotOptions {
                port,
                wait_time_ms: wait_time,
                env_vars,
                ..Default::default()
            };

            // connect to Dagger with options
            let logger = if cli.verbose {
                Logger::Default
            } else {
                Logger::Silent
            };
            let opts = ConnectOpts::default()
                .with_logger(logger)
                .with_execute_timeout(Some(600));

            opts.connect(move |client| async move {
                let app_dirs: Vec<_> = app_sources
                    .iter()
                    .map(|path| client.host().directory(path))
                    .collect();

                let num_apps = app_dirs.len();
                tracing::info!("Screenshotting {} apps in batch", num_apps);

                let screenshots_dir =
                    screenshot_apps_batch(&client, app_dirs, options, concurrency).await?;

                tracing::info!("Exporting screenshots to: {}", output);
                screenshots_dir.export(&output).await?;

                println!("✓ Screenshots saved to: {}", output);
                if num_apps == 1 {
                    println!("  Check app-0/ subdirectory");
                } else {
                    println!("  Check app-0/ through app-{}/ subdirectories", num_apps - 1);
                }
                Ok(())
            })
            .await?;
        }
    }

    Ok(())
}

fn parse_env_vars(s: &str) -> Vec<(String, String)> {
    if s.is_empty() {
        return vec![];
    }

    s.split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) if !k.trim().is_empty() && !v.trim().is_empty() => {
                    Some((k.trim().to_string(), v.trim().to_string()))
                }
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_vars_empty() {
        let result = parse_env_vars("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_env_vars_single() {
        let result = parse_env_vars("KEY=VALUE");
        assert_eq!(result, vec![("KEY".to_string(), "VALUE".to_string())]);
    }

    #[test]
    fn test_parse_env_vars_multiple() {
        let result = parse_env_vars("KEY1=VALUE1,KEY2=VALUE2");
        assert_eq!(
            result,
            vec![
                ("KEY1".to_string(), "VALUE1".to_string()),
                ("KEY2".to_string(), "VALUE2".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_env_vars_with_whitespace() {
        let result = parse_env_vars(" KEY = VALUE ");
        assert_eq!(result, vec![("KEY".to_string(), "VALUE".to_string())]);
    }

    #[test]
    fn test_parse_env_vars_invalid() {
        let result = parse_env_vars("INVALID,ALSO_INVALID");
        assert_eq!(result.len(), 0);
    }
}

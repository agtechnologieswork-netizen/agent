use super::agent::{Agent, AgentState, Event, EventHandler};
use super::replay::SandboxReplayer;
use super::tools::TemplateConfig;
use crate::toolbox::ToolDyn;
use dabgent_mq::{Envelope, EventStore, Handler};
use dabgent_sandbox::{Sandbox, SandboxHandle};
use eyre::Result;
use std::path::Path;

/// Handler that exports artifacts from the sandbox when the agent task is finished.
/// It replays all tool calls to rebuild the sandbox state, then exports the
/// artifacts using git-aware export (respecting .gitignore).
pub struct FinishHandler {
    sandbox_handle: SandboxHandle,
    export_path: String,
    tools: Vec<Box<dyn ToolDyn>>,
    template_config: TemplateConfig,
}

impl FinishHandler {
    pub fn new(
        sandbox_handle: SandboxHandle,
        export_path: String,
        tools: Vec<Box<dyn ToolDyn>>,
        template_config: TemplateConfig,
    ) -> Self {
        Self {
            sandbox_handle,
            export_path,
            tools,
            template_config,
        }
    }

    async fn replay_and_export<A: Agent, ES: EventStore>(
        &mut self,
        handler: &Handler<AgentState<A>, ES>,
        aggregate_id: &str,
    ) -> Result<()> {
        tracing::info!("Starting artifact export for aggregate: {}", aggregate_id);

        // Try to get existing sandbox, or create a fresh one from template
        let mut sandbox = match self.sandbox_handle.get(aggregate_id).await? {
            Some(s) => {
                tracing::info!("Found existing sandbox for aggregate: {}", aggregate_id);
                s
            }
            None => {
                tracing::info!(
                    "Creating fresh sandbox from template for aggregate: {}",
                    aggregate_id
                );
                self.sandbox_handle
                    .create_from_directory(
                        aggregate_id,
                        &self.template_config.host_dir,
                        &self.template_config.dockerfile,
                    )
                    .await?
            }
        };

        // Load all events for this aggregate
        let envelopes = handler.store().load_events::<AgentState<A>>(aggregate_id).await?;
        let events: Vec<Event<A::AgentEvent>> = envelopes.into_iter().map(|e| e.data).collect();

        // Replay all tool calls to rebuild complete sandbox state
        // This ensures we have all files created/modified during execution
        tracing::info!("Replaying {} events to rebuild sandbox state", events.len());
        let mut replayer = SandboxReplayer::new(&mut sandbox, &self.tools);
        replayer.apply_all(&events).await?;

        // Export artifacts using git-aware export
        self.export_artifacts(&mut sandbox).await?;

        // Clean up: we don't store the sandbox back since export is the final step
        tracing::info!("Artifact export completed for aggregate: {}", aggregate_id);

        Ok(())
    }

    async fn export_artifacts(&mut self, sandbox: &mut dabgent_sandbox::DaggerSandbox) -> Result<String> {
        tracing::info!(
            "Exporting artifacts (git-aware) from /app to {}",
            self.export_path
        );

        // Ensure export directory exists
        if let Some(parent) = Path::new(&self.export_path).parent() {
            std::fs::create_dir_all(parent)?;
        } else {
            std::fs::create_dir_all(&self.export_path)?;
        }

        // Deterministic git-based export: build /output inside sandbox, then export it
        // 1) Prepare output directory
        tracing::debug!("Preparing /output directory in sandbox");
        let prep = sandbox.exec("rm -rf /output && mkdir -p /output").await?;
        if prep.exit_code != 0 {
            tracing::error!("Failed to prepare /output: stderr={}, stdout={}", prep.stderr, prep.stdout);
            eyre::bail!("Failed to prepare /output: {}", prep.stderr);
        }

        // 2) Check if /app exists and has content
        let check_app = sandbox.exec("ls -la /app 2>&1 || echo 'no /app dir'").await?;
        tracing::debug!("Contents of /app: {}", check_app.stdout);

        // 3) Initialize git and stage non-ignored files
        tracing::debug!("Initializing git repository in /app");
        let git_commands = [
            ("git init", "git -C /app init"),
            ("git config user.email", "git -C /app config user.email agent@appbuild.com"),
            ("git config user.name", "git -C /app config user.name Agent"),
            ("git add", "git -C /app add -A"),
        ];

        for (desc, cmd) in git_commands {
            let res = sandbox.exec(cmd).await?;
            if res.exit_code != 0 {
                tracing::warn!("{} returned non-zero: stderr={}, stdout={}", desc, res.stderr, res.stdout);
                // Don't fail immediately, log and continue
                if !res.stderr.contains("already exists") && !res.stderr.is_empty() {
                    eyre::bail!("Git command failed ({}): stderr={}, stdout={}", cmd, res.stderr, res.stdout);
                }
            }
        }

        // 4) Populate /output from the index (respects .gitignore)
        tracing::debug!("Checking out files to /output");
        let checkout = sandbox
            .exec("git -C /app checkout-index --all --prefix=/output/ 2>&1")
            .await?;
        if checkout.exit_code != 0 {
            tracing::error!("git checkout-index failed: {}", checkout.stderr);
            // Try a fallback: just copy everything
            tracing::warn!("Falling back to direct copy of /app to /output");
            let fallback = sandbox.exec("cp -r /app/* /output/ 2>&1 || true").await?;
            tracing::debug!("Fallback copy result: {}", fallback.stdout);
        }

        // 5) Verify /output has content
        let check_output = sandbox.exec("ls -la /output").await?;
        tracing::info!("Contents of /output before export: {}", check_output.stdout);

        // 6) Export /output
        tracing::info!("Exporting /output to {}", self.export_path);
        sandbox
            .export_directory("/output", &self.export_path)
            .await?;

        tracing::info!("Artifacts exported successfully to {}", self.export_path);
        Ok(self.export_path.clone())
    }
}

impl<A: Agent, ES: EventStore> EventHandler<A, ES> for FinishHandler {
    async fn process(
        &mut self,
        handler: &Handler<AgentState<A>, ES>,
        envelope: &Envelope<AgentState<A>>,
    ) -> Result<()> {
        // Look for agent-specific Finished events
        if let Event::Agent(_) = &envelope.data {
            // Check if this is a "Finished" event by examining the event type
            use dabgent_mq::Event as MQEvent;
            let event_type = envelope.data.event_type();
            if event_type.contains("finished") || event_type.contains("done") {
                tracing::info!("Agent finished, starting artifact export");

                if let Err(e) = self.replay_and_export(handler, &envelope.aggregate_id).await {
                    tracing::error!("Failed to export artifacts: {}", e);
                }
            }
        }

        Ok(())
    }
}
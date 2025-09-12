use dabgent_agent::toolbox::Validator;
use dabgent_agent::llm::LLMClientDyn;
use dabgent_agent::utils::compact_error_message;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct DataAppsValidator {
    pub llm_client: Option<Arc<dyn LLMClientDyn>>,
    pub model: Option<String>,
}

impl Default for DataAppsValidator {
    fn default() -> Self {
        Self {
            llm_client: None,
            model: None,
        }
    }
}

impl DataAppsValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_llm_client(mut self, llm_client: Arc<dyn LLMClientDyn>, model: String) -> Self {
        self.llm_client = Some(llm_client);
        self.model = Some(model);
        self
    }

    async fn compact_error_if_needed(&self, error: &str) -> String {
        const MAX_ERROR_LENGTH: usize = 4096;

        dbg!(error);
        tracing::warn!("Original error: {}", error);

        if error.len() <= MAX_ERROR_LENGTH {
            return error.to_string();
        }

        if let (Some(llm), Some(model)) = (&self.llm_client, &self.model) {
            match compact_error_message(llm.as_ref(), model, error, MAX_ERROR_LENGTH).await {
                Ok(compacted) => {
                    tracing::info!("Compacted validation error: {} -> {} chars", error.len(), compacted.len());
                    compacted
                }
                Err(e) => {
                    tracing::warn!("Failed to compact error message: {}", e);
                    // Fallback to truncation
                    format!("{}...\n[Error compaction failed, truncated from {} characters]",
                        &error[..MAX_ERROR_LENGTH.saturating_sub(100)],
                        error.len())
                }
            }
        } else {
            // No LLM client configured, fallback to truncation
            format!("{}...\n[Truncated from {} characters]",
                &error[..MAX_ERROR_LENGTH.saturating_sub(50)],
                error.len())
        }
    }

    async fn check_python_dependencies(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<(), String> {
        // Try to install dependencies - need to be in backend directory for uv sync
        let result = sandbox.exec("uv sync --dev")
            .await.map_err(|e| format!("Failed to run uv sync: {}", e))?;

        tracing::info!("uv sync result: exit_code={}, stdout={}, stderr={}", result.exit_code, result.stdout, result.stderr);

        if result.exit_code != 0 {
            let error_msg = format!(
                "Python dependency installation failed (exit code {}): stderr: {} stdout: {}",
                result.exit_code,
                result.stderr,
                result.stdout
            );
            let compacted_error = self.compact_error_if_needed(&error_msg).await;
            return Err(compacted_error);
        }

        Ok(())
    }


    async fn check_linting(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<(), String> {
        sandbox.set_workdir("/app/backend").await.map_err(|e| format!("Failed to set workdir: {}", e))?;
        let result = sandbox.exec("uv run ruff check . --select E,W,F")
            .await.map_err(|e| format!("Failed to run linter: {}", e))?;

        if result.exit_code != 0 {
            let error_msg = format!(
                "Linting errors found (exit code {}): stderr: {} stdout: {}",
                result.exit_code,
                result.stderr,
                result.stdout
            );
            let compacted_error = self.compact_error_if_needed(&error_msg).await;
            return Err(compacted_error);
        }

        Ok(())
    }

    async fn check_frontend_build(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<(), String> {
        // Check if package.json exists
        let package_check = sandbox.read_file("/app/package.json").await;
        if package_check.is_err() {
            return Err("package.json not found in project root".to_string());
        }

        // Install npm dependencies
        sandbox.set_workdir("/app").await.map_err(|e| format!("Failed to set workdir: {}", e))?;
        let install_result = sandbox.exec("npm ci")
            .await.map_err(|e| format!("Failed to install npm dependencies: {}", e))?;

        if install_result.exit_code != 0 {
            let error_msg = format!(
                "npm install failed (exit code {}): stderr: {} stdout: {}",
                install_result.exit_code,
                install_result.stderr,
                install_result.stdout
            );
            let compacted_error = self.compact_error_if_needed(&error_msg).await;
            return Err(compacted_error);
        }

        // Build frontend
        let build_result = sandbox.exec("npm run build")
            .await.map_err(|e| format!("Failed to build frontend: {}", e))?;

        if build_result.exit_code != 0 {
            let error_msg = format!(
                "Frontend build failed (exit code {}): stderr: {} stdout: {}",
                build_result.exit_code,
                build_result.stderr,
                build_result.stdout
            );
            let compacted_error = self.compact_error_if_needed(&error_msg).await;
            return Err(compacted_error);
        }

        Ok(())
    }

    async fn check_tests(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<(), String> {
        let result = sandbox.exec("uv run pytest . -v")
            .await.map_err(|e| format!("Failed to run tests: {}", e))?;

        if result.exit_code != 0 {
            let error_msg = format!(
                "Tests failed (exit code {}): stderr: {} stdout: {}",
                result.exit_code,
                result.stderr,
                result.stdout
            );
            let compacted_error = self.compact_error_if_needed(&error_msg).await;
            return Err(compacted_error);
        }

        Ok(())
    }

    async fn export_requirements(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<(), String> {
        let result = sandbox.exec("uv export --no-hashes --format requirements-txt --output-file requirements.txt --no-dev")
            .await.map_err(|e| format!("Failed to run uv export: {}", e))?;

        if result.exit_code != 0 {
            let error_msg = format!(
                "uv export command failed (exit code {}): stderr: {} stdout: {}",
                result.exit_code,
                result.stderr,
                result.stdout
            );
            let compacted_error = self.compact_error_if_needed(&error_msg).await;
            return Err(compacted_error);
        }

        Ok(())
    }
}

impl Validator for DataAppsValidator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {

        // Initial setup: ensure we're in the backend directory and sync dependencies
        match sandbox.set_workdir("/app/backend").await {
            Ok(_) => (),
            Err(e) => return Ok(Err(format!("Failed to set workdir: {}", e))),
        };
        match sandbox.exec("uv sync --dev").await {
            Ok(_) => (),
            Err(e) => return Ok(Err(format!("Failed to run uv sync: {}", e))),
        }
        tracing::info!("Sandbox is ready. Starting validation steps...");

        // 1. Check Python dependencies
        if let Err(e) = self.check_python_dependencies(sandbox).await {
            return Ok(Err(format!("Dependency check failed: {}", e)));
        }

        // 2. Run smoke tests (includes import validation)
        if let Err(e) = self.check_tests(sandbox).await {
            return Ok(Err(format!("Smoke tests failed: {}", e)));
        }

        // 3. Check linting
        if let Err(e) = self.check_linting(sandbox).await {
            return Ok(Err(format!("Linting failed: {}", e)));
        }

        // 4. Check frontend build
        // if let Err(e) = self.check_frontend_build(sandbox).await {
        //     return Ok(Err(format!("Frontend build failed: {}", e)));
        // }

        // 5. Export requirements.txt for old-style projects if needed
        if self.export_requirements(sandbox).await.is_err() {
            return Ok(Err("Failed to export requirements.txt".to_string()));
        }
        Ok(Ok(()))
    }
}

use crate::toolbox;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;

/// Default validator for Python projects using uv
#[derive(Clone, Debug)]
pub struct PythonUvValidator;

impl toolbox::Validator for PythonUvValidator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        let result = sandbox.exec("uv run main.py").await?;
        Ok(match result.exit_code {
            0 | 124 => Ok(()), // 0 = success, 124 = timeout (considered success)
            code => Err(format!(
                "Validation failed with exit code: {}\nstdout: {}\nstderr: {}",
                code, result.stdout, result.stderr
            )),
        })
    }
}

/// Custom validator that runs a specific command
#[derive(Clone, Debug)]
pub struct CustomValidator {
    command: String,
}

impl CustomValidator {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }
}

impl toolbox::Validator for CustomValidator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        let result = sandbox.exec(&self.command).await?;
        Ok(match result.exit_code {
            0 => Ok(()),
            code => Err(format!(
                "Command '{}' failed with exit code: {}\nstdout: {}\nstderr: {}",
                self.command, code, result.stdout, result.stderr
            )),
        })
    }
}

/// No-op validator for cases where validation is not needed
#[derive(Clone, Debug)]
pub struct NoOpValidator;

impl toolbox::Validator for NoOpValidator {
    async fn run(&self, _sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        Ok(Ok(()))
    }
}
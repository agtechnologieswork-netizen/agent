use crate::{ExecResult, Sandbox};
use eyre::Result;

/// A sandbox implementation that performs no operations and always succeeds.
#[derive(Clone, Debug, Default)]
pub struct NoOpSandbox;

impl NoOpSandbox {
    pub fn new() -> Self {
        Self
    }
}

impl Sandbox for NoOpSandbox {
    async fn exec(&mut self, _command: &str) -> Result<ExecResult> {
        Ok(ExecResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    async fn write_file(&mut self, _path: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    async fn write_files(&mut self, _files: Vec<(&str, &str)>) -> Result<()> {
        Ok(())
    }

    async fn read_file(&self, _path: &str) -> Result<String> {
        Ok(String::new())
    }

    async fn delete_file(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn list_directory(&self, _path: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn set_workdir(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }

    async fn export_directory(&self, _container_path: &str, _host_path: &str) -> Result<String> {
        Ok(String::new())
    }

    async fn fork(&self) -> Result<Self> {
        Ok(Self)
    }
}

pub mod commands;
pub mod dagger;
use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: isize,
    pub stdout: String,
    pub stderr: String,
}

pub trait Sandbox {
    fn exec(&mut self, command: &str) -> impl Future<Output = Result<ExecResult>> + Send;
    fn write_file(&mut self, path: &str, content: &str) -> impl Future<Output = Result<()>> + Send;
    fn read_file(&self, path: &str) -> impl Future<Output = Result<String>> + Send;
    fn delete_file(&mut self, path: &str) -> impl Future<Output = Result<()>> + Send;
    fn list_directory(&self, path: &str) -> impl Future<Output = Result<Vec<String>>> + Send;
}

pub trait SandboxFork {
    fn fork(&self) -> impl Future<Output = Result<Self>> + Send
    where
        Self: Sized;
}

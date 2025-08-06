use serde::{Deserialize, Serialize};
use std::pin::Pin;
pub mod dagger;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bash(pub Vec<String>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WriteFile {
    pub path: String,
    pub contents: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadFile(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LsDir(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RmFile(pub String);

/// Low level commands. Workspace implementation can choose to serialize
/// these commands for persistence and replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Bash(Bash),
    WriteFile(WriteFile),
    ReadFile(ReadFile),
    LsDir(LsDir),
    RmFile(RmFile),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: isize,
    pub stdout: String,
    pub stderr: String,
}

pub trait Workspace: Send + Sync {
    fn bash(&mut self, cmd: Bash) -> impl Future<Output = eyre::Result<ExecResult>> + Send + Sync;
    fn write_file(
        &mut self,
        cmd: WriteFile,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync;
    fn read_file(
        &mut self,
        cmd: ReadFile,
    ) -> impl Future<Output = eyre::Result<String>> + Send + Sync;
    fn ls(&mut self, cmd: LsDir) -> impl Future<Output = eyre::Result<Vec<String>>> + Send + Sync;
    fn rm(&mut self, cmd: RmFile) -> impl Future<Output = eyre::Result<()>> + Send + Sync;
    fn fork(&self) -> impl Future<Output = eyre::Result<Self>> + Send + Sync
    where
        Self: Sized + 'static;
    fn boxed(self) -> Box<dyn WorkspaceDyn>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

pub trait WorkspaceDyn: Send + Sync {
    fn bash(
        &mut self,
        cmd: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<ExecResult>> + Send + Sync + '_>>;
    fn write_file(
        &mut self,
        path: &str,
        contents: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send + Sync + '_>>;
    fn read_file(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<String>> + Send + Sync + '_>>;
    fn ls(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<String>>> + Send + Sync + '_>>;
    fn rm(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send + Sync + '_>>;
    fn fork(
        &self,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Box<dyn WorkspaceDyn>>> + Send + Sync + '_>>;
}

impl<T: Workspace + 'static> WorkspaceDyn for T {
    fn bash(
        &mut self,
        cmd: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<ExecResult>> + Send + Sync + '_>> {
        let cmd = Bash(cmd.split_whitespace().map(String::from).collect());
        Box::pin(self.bash(cmd))
    }

    fn write_file(
        &mut self,
        path: &str,
        contents: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send + Sync + '_>> {
        let cmd = WriteFile {
            path: path.to_string(),
            contents: contents.to_string(),
        };
        Box::pin(self.write_file(cmd))
    }

    fn read_file(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<String>> + Send + Sync + '_>> {
        let cmd = ReadFile(path.to_string());
        Box::pin(self.read_file(cmd))
    }

    fn ls(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<String>>> + Send + Sync + '_>> {
        let cmd = LsDir(path.to_string());
        Box::pin(self.ls(cmd))
    }

    fn rm(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send + Sync + '_>> {
        let cmd = RmFile(path.to_string());
        Box::pin(self.rm(cmd))
    }

    fn fork(
        &self,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Box<dyn WorkspaceDyn>>> + Send + Sync + '_>> {
        Box::pin(async { self.fork().await.map(|x| x.boxed()) })
    }
}

pub struct WorkspaceVCR<T: Workspace> {
    inner: T,
    commands: Vec<Command>,
}

impl<T: Workspace> WorkspaceVCR<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            commands: Vec::new(),
        }
    }

    pub async fn load(mut inner: T, commands: Vec<Command>) -> eyre::Result<Self> {
        for cmd in commands.iter().cloned() {
            match cmd {
                Command::Bash(cmd) => {
                    inner.bash(cmd).await?;
                }
                Command::WriteFile(cmd) => {
                    inner.write_file(cmd).await?;
                }
                Command::ReadFile(cmd) => {
                    inner.read_file(cmd).await?;
                }
                Command::LsDir(cmd) => {
                    inner.ls(cmd).await?;
                }
                Command::RmFile(cmd) => {
                    inner.rm(cmd).await?;
                }
            };
        }
        Ok(Self { inner, commands })
    }
}

impl<T: Workspace + 'static> WorkspaceDyn for WorkspaceVCR<T> {
    fn bash(
        &mut self,
        cmd: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<ExecResult>> + Send + Sync + '_>> {
        let cmd = Bash(cmd.split_whitespace().map(String::from).collect());
        self.commands.push(Command::Bash(cmd.clone()));
        Box::pin(self.inner.bash(cmd))
    }

    fn write_file(
        &mut self,
        path: &str,
        contents: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send + Sync + '_>> {
        let cmd = WriteFile {
            path: path.to_string(),
            contents: contents.to_string(),
        };
        self.commands.push(Command::WriteFile(cmd.clone()));
        Box::pin(self.inner.write_file(cmd))
    }

    fn read_file(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<String>> + Send + Sync + '_>> {
        let cmd = ReadFile(path.to_string());
        self.commands.push(Command::ReadFile(cmd.clone()));
        Box::pin(self.inner.read_file(cmd))
    }

    fn ls(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Vec<String>>> + Send + Sync + '_>> {
        let cmd = LsDir(path.to_string());
        self.commands.push(Command::LsDir(cmd.clone()));
        Box::pin(self.inner.ls(cmd))
    }

    fn rm(
        &mut self,
        path: &str,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<()>> + Send + Sync + '_>> {
        let cmd = RmFile(path.to_string());
        self.commands.push(Command::RmFile(cmd.clone()));
        Box::pin(self.inner.rm(cmd))
    }

    fn fork(
        &self,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Box<dyn WorkspaceDyn>>> + Send + Sync + '_>> {
        Box::pin(async {
            let fork: Box<dyn WorkspaceDyn> = Box::new(WorkspaceVCR {
                inner: Workspace::fork(&self.inner).await?,
                commands: self.commands.clone(),
            });
            Ok(fork)
        })
    }
}

use crate::ExecResult;
use eyre::Result;
use std::env;

#[derive(Clone)]
pub struct Sandbox {
    ctr: dagger_sdk::Container,
}

impl Sandbox {
    /// Create a sandbox from an existing Dagger container
    pub fn from_container(ctr: dagger_sdk::Container) -> Self {
        Self { ctr }
    }
}

const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 60;

impl crate::Sandbox for Sandbox {
    async fn exec(&mut self, command: &str) -> Result<ExecResult> {
        let ctr = self.ctr.clone();
        let mut command: Vec<String> = command.split_whitespace().map(String::from).collect();
        let secs: u64 = env::var("DAGGER_EXEC_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS);
        if secs > 0 {
            command = ["timeout", &format!("{}s", secs)]
                .into_iter()
                .map(String::from)
                .chain(command)
                .collect();
        }
        let opts = dagger_sdk::ContainerWithExecOptsBuilder::default()
            .expect(dagger_sdk::ReturnType::Any)
            .build()
            .unwrap();
        let ctr = ctr.with_exec_opts(command, opts);
        let res = ExecResult::get_output(&ctr).await?;
        self.ctr = ctr;
        Ok(res)
    }

    async fn write_file(&mut self, path: &str, content: &str) -> Result<()> {
        self.ctr = self.ctr.with_new_file(path, content);
        Ok(())
    }

    async fn read_file(&self, path: &str) -> Result<String> {
        self.ctr.file(path).contents().await.map_err(Into::into)
    }

    async fn delete_file(&mut self, path: &str) -> Result<()> {
        self.ctr = self.ctr.without_file(path);
        Ok(())
    }

    async fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        self.ctr.directory(path).entries().await.map_err(Into::into)
    }
}

impl crate::SandboxFork for Sandbox {
    async fn fork(&self) -> Result<Self>
    where
        Self: Sized,
    {
        let ctr = self.ctr.clone();
        Ok(Sandbox { ctr })
    }
}

impl ExecResult {
    async fn get_output(ctr: &dagger_sdk::Container) -> Result<Self> {
        Ok(Self {
            exit_code: ctr.exit_code().await?,
            stdout: ctr.stdout().await?,
            stderr: ctr.stderr().await?,
        })
    }
}

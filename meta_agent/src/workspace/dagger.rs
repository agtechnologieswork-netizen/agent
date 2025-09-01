use crate::workspace::*;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

/// Create a PostgreSQL service with unique instance ID.
fn create_postgres_service(client: &dagger_sdk::DaggerConn) -> dagger_sdk::Service {
    client
        .container()
        .from("postgres:17.0-alpine")
        .with_env_variable("POSTGRES_USER", "postgres")
        .with_env_variable("POSTGRES_PASSWORD", "postgres")
        .with_env_variable("POSTGRES_DB", "postgres")
        .with_env_variable("INSTANCE_ID", Uuid::new_v4().to_string())
        .as_service()
}

/// A reference to a Dagger connection that can be used to create and manage workspaces.
/// Created workspaces are valid for the lifetime of the DaggerRef instance.
#[derive(Clone)]
pub struct DaggerRef {
    sender: mpsc::Sender<oneshot::Sender<dagger_sdk::DaggerConn>>,
}

impl DaggerRef {
    pub fn new() -> Self {
        Self::with_verbose(std::env::var("DAGGER_VERBOSE").is_ok())
    }

    pub fn with_verbose(verbose: bool) -> Self {
        let (sender, mut receiver) = mpsc::channel::<oneshot::Sender<dagger_sdk::DaggerConn>>(1);
        tokio::spawn(async move {
            let logger = if verbose { 
                dagger_sdk::core::config::Config::default().logger 
            } else { 
                None 
            };
            let config = dagger_sdk::core::config::Config::new(None, None, None, None, logger);
            let _ = dagger_sdk::connect_opts(config, |client| async move {
                while let Some(reply) = receiver.recv().await {
                    let _ = reply.send(client.clone());
                }
                Ok(())
            })
            .await;
        });
        Self { sender }
    }

    pub async fn client(&self) -> eyre::Result<dagger_sdk::DaggerConn> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.sender.send(reply_tx).await;
        reply_rx.await.map_err(Into::into)
    }

    pub async fn workspace(
        &self,
        dockerfile: String,
        context: String,
    ) -> eyre::Result<DaggerWorkspace> {
        let opts = dagger_sdk::ContainerBuildOptsBuilder::default()
            .dockerfile(dockerfile.as_str())
            .build()?;
        let client = self.client().await?;
        let ctr = client
            .container()
            .build_opts(client.host().directory(context), opts);
        ctr.sync().await?; // Eagerly evaluate and fail if workspace is invalid
        Ok(DaggerWorkspace { ctr, client })
    }
}

impl Default for DaggerRef {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct DaggerWorkspace {
    ctr: dagger_sdk::Container,
    client: dagger_sdk::DaggerConn, // Add client reference for creating services
}

impl DaggerWorkspace {
    async fn exec_res(ctr: &dagger_sdk::Container) -> eyre::Result<ExecResult> {
        Ok(ExecResult {
            exit_code: ctr.exit_code().await?,
            stdout: ctr.stdout().await?,
            stderr: ctr.stderr().await?,
        })
    }

    /// Write multiple files to the container in a single operation to prevent deep query chains
    async fn write_files_bulk(&mut self, files: Vec<(String, String)>) -> eyre::Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let mut ctr = self.ctr.clone();
        for (path, contents) in files {
            ctr = ctr.with_new_file(path, contents);
        }
        
        self.ctr = ctr;
        Ok(())
    }

    async fn bash_with_pg_impl(&mut self, cmd: &str) -> eyre::Result<ExecResult> {
        let cmd_args: Vec<String> = cmd.split_whitespace().map(String::from).collect();
        let postgres_service = create_postgres_service(&self.client);
        let ctr = self.ctr.clone();
        
        let (res, ctr) = tokio::spawn(async move {
            let opts = dagger_sdk::ContainerWithExecOptsBuilder::default()
                .expect(dagger_sdk::ReturnType::Any)
                .build()
                .unwrap();
                
            let ctr = ctr
                .with_exec(vec!["apt-get".to_string(), "update".to_string()])
                .with_exec(vec!["apt-get".to_string(), "install".to_string(), "-y".to_string(), "postgresql-client".to_string()])
                .with_service_binding("postgres", postgres_service)
                .with_env_variable("APP_DATABASE_URL", "postgresql://postgres:postgres@postgres:5432/postgres")
                .with_exec(vec!["sh".to_string(), "-c".to_string(), "while ! pg_isready -h postgres -U postgres; do sleep 1; done".to_string()])
                .with_exec_opts(cmd_args, opts);
                
            let res = DaggerWorkspace::exec_res(&ctr).await;
            (res, ctr)
        })
        .await.map_err(|e| {
            tracing::error!("PostgreSQL service binding failed - DNS resolution issue likely: {}", e);
            eyre::eyre!("PostgreSQL execution failed (DNS resolution issue): {}", e)
        })?;
        
        self.ctr = ctr;
        res
    }
}

impl Workspace for DaggerWorkspace {
    async fn bash(&mut self, cmd: Bash) -> eyre::Result<ExecResult> {
        let ctr = self.ctr.clone();
        let (res, ctr) = tokio::spawn(async move {
            let opts = dagger_sdk::ContainerWithExecOptsBuilder::default()
                .expect(dagger_sdk::ReturnType::Any)
                .build()
                .unwrap();
            let ctr = ctr.with_exec_opts(cmd.0, opts);
            let res = DaggerWorkspace::exec_res(&ctr).await;
            (res, ctr)
        })
        .await?;
        self.ctr = ctr;
        res
    }

    async fn write_file(&mut self, cmd: WriteFile) -> eyre::Result<()> {
        self.ctr = self.ctr.with_new_file(cmd.path, cmd.contents);
        Ok(())
    }

    async fn read_file(&mut self, cmd: ReadFile) -> eyre::Result<String> {
        let ctr = self.ctr.clone();
        let res = tokio::spawn(async move { ctr.file(cmd.0).contents().await }).await?;
        res.map_err(Into::into)
    }

    async fn ls(&mut self, cmd: LsDir) -> eyre::Result<Vec<String>> {
        let ctr = self.ctr.clone();
        let res = tokio::spawn(async move { ctr.directory(cmd.0).entries().await }).await?;
        res.map_err(Into::into)
    }

    async fn rm(&mut self, cmd: RmFile) -> eyre::Result<()> {
        self.ctr = self.ctr.without_file(cmd.0);
        Ok(())
    }

    async fn fork(&self) -> eyre::Result<Self> {
        let ctr = self.ctr.clone();
        let client = self.client.clone();
        Ok(DaggerWorkspace { ctr, client })
    }
}

// Override WorkspaceDyn to provide PostgreSQL support
impl crate::workspace::WorkspaceDyn for DaggerWorkspace {
    fn bash(
        &mut self,
        cmd: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<ExecResult>> + Send + Sync + '_>> {
        let cmd = Bash(cmd.split_whitespace().map(String::from).collect());
        Box::pin(async move { Workspace::bash(self, cmd).await })
    }
    
    fn bash_with_pg(
        &mut self,
        cmd: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<ExecResult>> + Send + Sync + '_>> {
        let cmd = cmd.to_string();
        Box::pin(async move { self.bash_with_pg_impl(&cmd).await })
    }
    
    fn write_file(
        &mut self,
        path: &str,
        contents: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<()>> + Send + Sync + '_>> {
        let cmd = WriteFile {
            path: path.to_string(),
            contents: contents.to_string(),
        };
        Box::pin(async move { Workspace::write_file(self, cmd).await })
    }
    
    fn read_file(
        &mut self,
        path: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<String>> + Send + Sync + '_>> {
        let cmd = ReadFile(path.to_string());
        Box::pin(async move { Workspace::read_file(self, cmd).await })
    }
    
    fn ls(
        &mut self,
        path: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<Vec<String>>> + Send + Sync + '_>> {
        let cmd = LsDir(path.to_string());
        Box::pin(async move { Workspace::ls(self, cmd).await })
    }
    
    fn rm(
        &mut self,
        path: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<()>> + Send + Sync + '_>> {
        let cmd = RmFile(path.to_string());
        Box::pin(async move { Workspace::rm(self, cmd).await })
    }
    
    fn fork(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<Box<dyn crate::workspace::WorkspaceDyn>>> + Send + Sync + '_>> {
        Box::pin(async move {
            let forked = Workspace::fork(self).await?;
            Ok(Box::new(forked) as Box<dyn crate::workspace::WorkspaceDyn>)
        })
    }
    
    fn write_files_bulk(
        &mut self,
        files: Vec<(String, String)>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = eyre::Result<()>> + Send + Sync + '_>> {
        Box::pin(async move { 
            DaggerWorkspace::write_files_bulk(self, files).await 
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    const TEST_DOCKERFILE: &str = "Dockerfile.appbuild";

    async fn setup_workspace(dagger_ref: &DaggerRef) -> DaggerWorkspace {
        let temp_dir = TempDir::new("dagger").unwrap();
        let docker_path = temp_dir.path().join(TEST_DOCKERFILE);
        let dir_path = temp_dir.path().to_str().unwrap().to_string();
        std::fs::write(docker_path, "FROM alpine:latest\n").unwrap();
        let workspace = dagger_ref.workspace(TEST_DOCKERFILE.to_string(), dir_path);
        workspace.await.unwrap()
    }

    #[tokio::test]
    async fn test_dagger_workspace() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        let bash_cmd = Bash(vec!["echo".to_string(), "Hello World!".to_string()]);
        let result = Workspace::bash(&mut workspace, bash_cmd).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "Hello World!");
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_dagger_parallel() {
        let dagger_ref = DaggerRef::new();
        let concurrent_tasks = 10;
        let mut set = tokio::task::JoinSet::new();
        for i in 0..concurrent_tasks {
            let mut workspace = setup_workspace(&dagger_ref).await;
            set.spawn(async move {
                let bash_cmd = Bash(vec!["echo".to_string(), i.to_string()]);
                (Workspace::bash(&mut workspace, bash_cmd).await, i)
            });
        }
        let mut results = Vec::new();
        while let Some(res) = set.join_next().await {
            results.push(res.unwrap());
        }
        assert_eq!(results.len(), concurrent_tasks);
        for (result, i) in results {
            assert!(result.is_ok(), "Task {i} failed: {:?}", result);
            let result = result.unwrap();
            assert_eq!(result.exit_code, 0);
            assert_eq!(result.stdout.trim(), i.to_string());
            assert!(result.stderr.is_empty());
        }
    }

    #[tokio::test]
    async fn test_bulk_write() {
        let dagger_ref = DaggerRef::new();
        let mut workspace = setup_workspace(&dagger_ref).await;
        
        // Prepare bulk files (simulate template files scenario)
        let files = vec![
            ("file1.txt".to_string(), "content1".to_string()),
            ("file2.txt".to_string(), "content2".to_string()),
            ("dir/file3.txt".to_string(), "content3".to_string()),
            ("dir/file4.txt".to_string(), "content4".to_string()),
            ("another/deep/file5.txt".to_string(), "content5".to_string()),
        ];
        
        // Write files in bulk
        workspace.write_files_bulk(files).await.unwrap();
        
        // Verify files were written correctly by reading them back
        let content1 = Workspace::read_file(&mut workspace, ReadFile("file1.txt".to_string())).await.unwrap();
        assert_eq!(content1, "content1");
        
        let content2 = Workspace::read_file(&mut workspace, ReadFile("file2.txt".to_string())).await.unwrap();
        assert_eq!(content2, "content2");
        
        let content5 = Workspace::read_file(&mut workspace, ReadFile("another/deep/file5.txt".to_string())).await.unwrap();
        assert_eq!(content5, "content5");
    }
}

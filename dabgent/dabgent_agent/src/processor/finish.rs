use dabgent_sandbox::SandboxDyn;
use eyre::Result;
use std::future::Future;

pub trait ArtifactPreparer: Send + Sync {
    fn prepare(&self, sandbox: &mut Box<dyn SandboxDyn>) -> impl Future<Output = Result<()>> + Send;
}
use crate::dagger::Sandbox as DaggerSandbox;
use eyre::Result;

/// Create a Dagger sandbox with specified context and dockerfile
pub async fn create_sandbox(
    client: &dagger_sdk::DaggerConn,
    context_dir: &str,
    dockerfile: &str,
) -> Result<DaggerSandbox> {
    let ctr = client.container().build_opts(
        client.host().directory(context_dir),
        dagger_sdk::ContainerBuildOptsBuilder::default()
            .dockerfile(dockerfile)
            .build()?
    );
    ctr.sync().await?;
    Ok(DaggerSandbox::from_container(ctr))
}
use crate::workspace::*;

pub struct MockWorkspace;

impl Workspace for MockWorkspace {
    async fn bash(&mut self, _cmd: Bash) -> eyre::Result<ExecResult> {
        unimplemented!();
    }

    async fn write_file(&mut self, _cmd: WriteFile) -> eyre::Result<()> {
        unimplemented!();
    }

    async fn read_file(&mut self, _cmd: ReadFile) -> eyre::Result<String> {
        unimplemented!();
    }

    async fn ls(&mut self, _cmd: LsDir) -> eyre::Result<Vec<String>> {
        unimplemented!();
    }

    async fn rm(&mut self, _cmd: RmFile) -> eyre::Result<()> {
        unimplemented!();
    }

    async fn fork(&self) -> eyre::Result<Self> {
        unimplemented!();
    }
}

pub fn default_mock() -> Box<dyn WorkspaceDyn> {
    Box::new(MockWorkspace)
}

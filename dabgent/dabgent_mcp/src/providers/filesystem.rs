use eyre::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_TEMPLATE_PATH: &str = "../../dataapps/template_minimal";

#[derive(Clone)]
pub struct FilesystemProvider {
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct InitiateProjectArgs {
    /// Path to the work directory to copy to
    pub work_dir: String,
    /// If true, wipe the work directory before copying
    #[serde(default)]
    pub force_rewrite: bool,
}


#[tool_router]
impl FilesystemProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            tool_router: Self::tool_router(),
        })
    }

    #[tool(
        name = "initiate_project",
        description = "Initialize a project by copying template files from the default template to a work directory. Uses ../../dataapps/template_minimal as the template source. Supports force rewrite to wipe and recreate the directory."
    )]
    pub async fn initiate_project(
        &self,
        Parameters(args): Parameters<InitiateProjectArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        // use hardcoded template path
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let template_path = manifest_dir.join(DEFAULT_TEMPLATE_PATH);
        let work_path = PathBuf::from(&args.work_dir);

        // validate template directory exists
        if !template_path.exists() {
            return Err(ErrorData::internal_error(
                format!("default template directory does not exist: {}", template_path.display()),
                None,
            ));
        }

        if !template_path.is_dir() {
            return Err(ErrorData::internal_error(
                format!("default template path is not a directory: {}", template_path.display()),
                None,
            ));
        }

        // handle force rewrite
        if args.force_rewrite && work_path.exists() {
            std::fs::remove_dir_all(&work_path).map_err(|e| {
                ErrorData::internal_error(
                    format!("failed to remove work directory: {}", e),
                    None,
                )
            })?;
        }

        // create work directory if it doesn't exist
        std::fs::create_dir_all(&work_path).map_err(|e| {
            ErrorData::internal_error(format!("failed to create work directory: {}", e), None)
        })?;

        // collect and copy files using git ls-files
        let files = collect_template_files(&template_path, &work_path).map_err(|e| {
            ErrorData::internal_error(format!("failed to collect template files: {}", e), None)
        })?;

        let message = format!(
            "Successfully copied {} files from default template to {}",
            files.len(),
            args.work_dir
        );

        Ok(CallToolResult::success(vec![Content::text(message)]))
    }
}

fn collect_template_files(template_path: &Path, work_path: &Path) -> Result<Vec<PathBuf>> {
    use std::process::Command;

    let output = Command::new("git")
        .arg("-C")
        .arg(template_path)
        .arg("ls-files")
        .output()?;

    if !output.status.success() {
        eyre::bail!("git ls-files failed");
    }

    let files_list = String::from_utf8(output.stdout)?;
    let mut copied_files = Vec::new();

    for relative_path in files_list.lines() {
        if relative_path.is_empty() {
            continue;
        }

        let source_file = template_path.join(relative_path);
        let target_file = work_path.join(relative_path);

        // ensure parent directory exists
        if let Some(parent) = target_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // copy file
        std::fs::copy(&source_file, &target_file)?;
        copied_files.push(target_file);
    }

    Ok(copied_files)
}

#[tool_handler]
impl ServerHandler for FilesystemProvider {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "dabgent-mcp-filesystem".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Dabgent MCP - Filesystem".to_string()),
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "MCP server providing filesystem tools for project initialization and template management.".to_string(),
            ),
        }
    }
}

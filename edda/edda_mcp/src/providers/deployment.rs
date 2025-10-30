use crate::state;
use edda_integrations::{
    AppInfo, CreateApp, Resources, ToolResultDisplay, create_app, deploy_app, get_app_info,
    get_user_info, sync_workspace,
};
use eyre::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone)]
pub struct DeploymentProvider {
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeployDatabricksAppArgs {
    /// Absolute path to the work directory containing the app to deploy (e.g., /path/to/project)
    pub work_dir: String,
    /// Name of the Databricks app (alphanumeric and dash characters only)
    pub name: String,
    /// Description of the Databricks app
    pub description: String,
    /// Force re-deployment if the app already exists
    pub force: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployDatabricksAppResult {
    pub success: bool,
    pub message: String,
    pub app_url: Option<String>,
    pub app_name: String,
}

impl ToolResultDisplay for DeployDatabricksAppResult {
    fn display(&self) -> String {
        if self.success {
            format!(
                "Successfully deployed app '{}'\nURL: {}\n{}",
                self.app_name,
                self.app_url.as_ref().unwrap_or(&"N/A".to_string()),
                self.message
            )
        } else {
            format!(
                "Deployment failed for app '{}': {}",
                self.app_name, self.message
            )
        }
    }
}

#[tool_router]
impl DeploymentProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            tool_router: Self::tool_router(),
        })
    }

    /// Core logic for deploying a Databricks app
    async fn deploy_databricks_app_impl(
        work_dir: &str,
        name: &str,
        description: &str,
        force: bool,
    ) -> Result<DeployDatabricksAppResult> {
        let start_time = std::time::Instant::now();
        // Validate work directory exists
        let work_path = PathBuf::from(work_dir);
        if !work_path.exists() {
            return Ok(DeployDatabricksAppResult {
                success: false,
                message: format!("Work directory does not exist: {}", work_dir),
                app_url: None,
                app_name: name.to_string(),
            });
        }

        if !work_path.is_dir() {
            return Ok(DeployDatabricksAppResult {
                success: false,
                message: format!("Work path is not a directory: {}", work_dir),
                app_url: None,
                app_name: name.to_string(),
            });
        }

        // load and verify state
        let project_state = match state::load_state(&work_path)? {
            Some(state) => state,
            None => {
                return Ok(DeployDatabricksAppResult {
                    success: false,
                    message: "Project must be scaffolded before deployment".to_string(),
                    app_url: None,
                    app_name: name.to_string(),
                });
            }
        };

        // verify checksum hasn't changed since validation
        let expected_checksum = match project_state.checksum() {
            Some(checksum) => checksum,
            None => {
                return Ok(DeployDatabricksAppResult {
                    success: false,
                    message: "Project must be validated before deployment. Run validate_data_app first.".to_string(),
                    app_url: None,
                    app_name: name.to_string(),
                });
            }
        };

        match state::verify_checksum(&work_path, expected_checksum) {
            Ok(true) => {},
            Ok(false) => {
                return Ok(DeployDatabricksAppResult {
                    success: false,
                    message: "Project files changed since validation. Re-run validate_data_app before deployment.".to_string(),
                    app_url: None,
                    app_name: name.to_string(),
                });
            },
            Err(e) => {
                return Ok(DeployDatabricksAppResult {
                    success: false,
                    message: format!("Failed to verify project checksum: {}", e),
                    app_url: None,
                    app_name: name.to_string(),
                });
            }
        }

        // Install project dependencies
        run_format_cmd(
            std::process::Command::new("npm")
                .args(&["install"])
                .current_dir(&work_path),
        )?;

        // Build frontend
        run_format_cmd(
            std::process::Command::new("npm")
                .args(&["run", "build"])
                .current_dir(&work_path),
        )?;

        // Get or create app
        let app_info: AppInfo = match get_app_info(name) {
            Ok(info) => {
                tracing::info!("Found existing app: {}", name);
                let user_info =
                    get_user_info().map_err(|e| eyre::eyre!("Failed to get user info: {}", e))?;
                if (!force) && (info.creator != user_info.user_name) {
                    return Err(eyre::eyre!(
                        "App '{}' already exists and was created by another user: {}. Use 'force' option to override.",
                        name,
                        info.creator
                    ));
                }
                info
            }
            Err(_) => {
                tracing::info!("App not found, creating new app: {}", name);
                let resources = Resources::from_env()
                    .map_err(|e| eyre::eyre!("{}", e))?;
                let command = CreateApp::new(name, description).with_resources(resources);
                create_app(&command).map_err(|e| eyre::eyre!("Failed to create app: {}", e))?
            }
        };

        // Sync workspace
        let server_dir = format!("{work_dir}/server");
        let sync_start = std::time::Instant::now();
        tracing::info!("Syncing workspace from {} to Databricks", server_dir);
        sync_workspace(&app_info, &server_dir)
            .map_err(|e| eyre::eyre!("Failed to sync workspace: {}", e))?;
        let sync_duration = sync_start.elapsed().as_secs_f64();
        tracing::info!(duration = sync_duration, "Workspace sync completed");

        // Deploy app
        let deploy_start = std::time::Instant::now();
        tracing::info!("Deploying app: {}", name);
        deploy_app(&app_info).map_err(|e| eyre::eyre!("Failed to deploy app: {}", e))?;
        let deploy_duration = deploy_start.elapsed().as_secs_f64();
        tracing::info!(duration = deploy_duration, "App deployment completed");

        // transition to deployed state
        let project_state = project_state.deploy()?;
        state::save_state(&work_path, &project_state)?;

        let total_duration = start_time.elapsed().as_secs_f64();
        tracing::info!(duration = total_duration, "Full deployment completed");

        Ok(DeployDatabricksAppResult {
            success: true,
            message: "Deployment completed successfully".to_string(),
            app_url: Some(app_info.url.clone()),
            app_name: name.to_string(),
        })
    }

    #[tool(
        name = "deploy_databricks_app",
        description = "Deploy a generated app to Databricks Apps. Creates the app if it doesn't exist, syncs local files to workspace, and deploys the app. Returns deployment status and app URL. Only use after direct user request and running validation."
    )]
    pub async fn deploy_databricks_app(
        &self,
        Parameters(args): Parameters<DeployDatabricksAppArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let work_path = PathBuf::from(&args.work_dir);

        // Validate that the path is absolute
        if !work_path.is_absolute() {
            return Err(ErrorData::invalid_params(
                format!(
                    "work_dir must be an absolute path, got: '{}'. Relative paths are not supported",
                    args.work_dir
                ),
                None,
            ));
        }

        let result = Self::deploy_databricks_app_impl(
            &args.work_dir,
            &args.name,
            &args.description,
            args.force.unwrap_or(false),
        )
        .await
        .map_err(|e| ErrorData::internal_error(format!("Failed to deploy app: {}", e), None))?;

        if result.success {
            Ok(CallToolResult::success(vec![Content::text(
                result.display(),
            )]))
        } else {
            Err(ErrorData::internal_error(result.message, None))
        }
    }
}

#[tool_handler]
impl ServerHandler for DeploymentProvider {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "edda-mcp-deployment".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Edda MCP - Deployment".to_string()),
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "MCP server providing deployment tools for Databricks Apps.".to_string(),
            ),
        }
    }
}

fn run_format_cmd(command: &mut std::process::Command) -> Result<std::process::Output> {
    let output = command.output().map_err(|e| eyre::eyre!("Error: {e}"))?;
    if !output.status.success() {
        return Err(eyre::eyre!(
            "Error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

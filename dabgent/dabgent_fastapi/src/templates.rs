use rust_embed::Embed;
use eyre::Result;

pub const EMBEDDED_TEMPLATES: &str = "embedded://templates";
pub const DEFAULT_TEMPLATE_PATH: &str = "../dataapps/template_minimal";

#[derive(Embed)]
#[folder = "../../dataapps/template_minimal"]
#[exclude = "node_modules/*"]
#[exclude = ".git/*"]
#[exclude = ".venv/*"]
#[exclude = "target/*"]
#[exclude = "dist/*"]
#[exclude = "build/*"]
pub struct Templates;

/// Get template files for seeding sandbox
pub fn get_template_files(template_path: &str, base_path: &str) -> Result<Vec<(String, String)>> {
    if template_path == EMBEDDED_TEMPLATES {
        // Use embedded templates
        let mut files = Vec::new();
        for path in Templates::iter() {
            if let Some(file) = Templates::get(path.as_ref()) {
                let content = String::from_utf8_lossy(&file.data).into_owned();
                let sandbox_path = format!("{}/{}", base_path, path.as_ref());
                files.push((sandbox_path, content));
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(files)
    } else {
        // Use filesystem templates
        let template_files = dabgent_agent::sandbox_seed::collect_template_files(
            std::path::Path::new(template_path),
            base_path
        )?;
        Ok(template_files.files)
    }
}

/// Seed sandbox with templates - handles both embedded and filesystem templates
pub async fn seed_sandbox(
    sandbox: &mut Box<dyn dabgent_sandbox::SandboxDyn>,
    template_path: &str,
    base_path: &str,
) -> Result<(usize, String)> {
    let files = get_template_files(template_path, base_path)?;
    let hash = dabgent_agent::sandbox_seed::compute_template_hash(&files);
    let file_count = dabgent_agent::sandbox_seed::write_template_files(sandbox, &files).await?;
    Ok((file_count, hash))
}
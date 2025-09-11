use dabgent_agent::toolbox::{ToolDyn, Validator, basic::{WriteFile, ReadFile, EditFile, LsDir, RmFile, UvAdd, DoneTool}};

pub fn dataapps_toolset<T: Validator + Send + Sync + 'static>(validator: T) -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(WriteFile),
        Box::new(ReadFile),
        Box::new(EditFile),
        Box::new(LsDir),
        Box::new(RmFile),
        Box::new(UvAdd),
        Box::new(DoneTool::new(validator)),
    ]
}

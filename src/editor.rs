//! Native desktop editor entry point powered by cssimpler.
use marvelous_cloth::project::Project;
use std::path::PathBuf;

#[path = "editor/css_editor.rs"]
pub mod css_editor;

pub fn run(project: Project, output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    css_editor::run(project, output).map_err(|e| e.to_string().into())
}

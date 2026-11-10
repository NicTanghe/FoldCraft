//! Garment workspace: library | 3D | 2D patterns | properties.
use marvelous_cloth::fitting::{FittingProject, FittingScene};
use std::path::PathBuf;

#[path = "atelier/core_state.rs"]
pub mod core_state;
#[path = "atelier/css_runner.rs"]
pub mod css_runner;
#[allow(unused_imports)]
pub use core_state::*;
pub use marvelous_cloth::commands;
#[allow(unused_imports)]
pub use marvelous_cloth::css_ui;

// GPU and diagnostic entry points remain available during native adapter migration.
#[allow(dead_code)]
#[path = "atelier/worker.rs"]
pub mod worker;

#[allow(dead_code)]
pub struct DocumentFiles {
    pub output: PathBuf,
    pub source: Option<PathBuf>,
    pub modified: bool,
}

#[allow(dead_code)]
pub fn current_worker_result(done: Option<worker::Done>, generation: u64) -> Option<worker::Done> {
    done.filter(|d| d.generation == generation)
}

pub fn run(
    project: FittingProject,
    scene: FittingScene,
    files: DocumentFiles,
    smoke: bool,
    _drag_test: bool,
    _perf_test: bool,
    _authoring_test: bool,
) -> Result<(), String> {
    css_runner::run(project, scene, files, smoke)
}

#[path = "atelier/usd_smoke.rs"]
mod usd_smoke;

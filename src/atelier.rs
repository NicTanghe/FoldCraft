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
#[path = "atelier/perf.rs"]
pub mod perf;
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
    drag_test: bool,
    perf_test: bool,
    authoring_test: bool,
    gpu_sim: bool,
) -> Result<(), String> {
    let output = files.output.clone();
    css_runner::run(
        project,
        scene,
        files,
        smoke,
        drag_test,
        perf_test,
        authoring_test,
        gpu_sim,
    )?;
    if perf_test {
        let perf_report = output.join("ui-perf.json");
        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&perf_report)
                .map_err(|e| format!("Failed to read {}: {e}", perf_report.display()))?,
        )
        .map_err(|e| e.to_string())?;
        if report["passed"] != true {
            return Err(format!("UI realtime check failed: {report}"));
        }
    }
    Ok(())
}

#[path = "atelier/usd_smoke.rs"]
mod usd_smoke;

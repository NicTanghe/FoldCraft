//! Native CSSimpler Studio V2 Garment Workspace Runner for `cloth-fit --view`.
//!
//! The complete native CSSimpler Studio V2 garment workspace for `cloth-fit --view`.
//!
//! Provides:
//! - Real simulation execution backed by `FittingProject`, `FittingScene`, `VbdSettings`,
//!   and the simulation worker thread (`worker::start_cpu` / `worker::start`).
//! - Studio V2 5-row/4-column behavioral layout.
//! - Interactive 3D garment + avatar projection viewport with mouse drag orbit/zoom.
//! - 2D metric pattern board with boundary curves, seam lines, notches, ruler annotations, and grainline.
//! - Top menu & transport bar (Play/Pause, Step, Reset, Clean View, Command Palette).
//! - 3D & 2D tool rails + Corner Tool controls.
//! - Left Asset & Piece Browser (bundled templates catalog, piece cards, dirty doc state, import/export quick actions).
//! - Right 5-Category Object Browser (2D Pattern, Sewing, 3D Form, Material, Environment).
//! - Right Property Editor accordions (Information, Selected Line, Arrange, Measurements, Pins, Fabric & Appearance, Trims, Mesh Preparation, Avatar Diagnostics).
//! - Bottom status bar (step count, execution ms, FPS, vertices, triangles, status message).

#![allow(dead_code)]

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

use cssimpler::app::{App, Invalidation, RenderMode};
use cssimpler::core::Node;
use cssimpler::renderer::{EngineEvent, FrameInfo, WindowConfig};
use cssimpler::style::Stylesheet;
use cssimpler::ui;

use marvelous_cloth::css_ui::icons::*;
use marvelous_cloth::css_ui::{Theme, ViewLayoutMode, load_studio_stylesheet};
use marvelous_cloth::fitting::{FittingProject, FittingScene};
use marvelous_cloth::garment::export;
use marvelous_cloth::gpu_avbd::ContactMethod;
use marvelous_cloth::render::Camera;
use marvelous_cloth::vbd::{VbdMetrics, VbdSettings};

use super::DocumentFiles;
use super::worker::{self, Latest, Work};

// ============================================================================
// Atomic Action Flags
// ============================================================================

pub const ACT_RUN_PLAY_PAUSE: u64 = 1 << 0;
pub const ACT_RUN_STEP: u64 = 1 << 1;
pub const ACT_RUN_RESET: u64 = 1 << 2;
pub const ACT_RUN_SAVE: u64 = 1 << 3;
pub const ACT_RUN_EXPORT: u64 = 1 << 4;
pub const ACT_RUN_CLEAN_VIEW: u64 = 1 << 5;
pub const ACT_RUN_PALETTE_TOGGLE: u64 = 1 << 6;
pub const ACT_RUN_THEME_TOGGLE: u64 = 1 << 7;
pub const ACT_RUN_VIEW_SPLIT: u64 = 1 << 8;
pub const ACT_RUN_VIEW_3D: u64 = 1 << 9;
pub const ACT_RUN_VIEW_2D: u64 = 1 << 10;
pub const ACT_RUN_DRAWER_TOGGLE: u64 = 1 << 11;
pub const ACT_RUN_TAB_PATTERN: u64 = 1 << 12;
pub const ACT_RUN_TAB_SEWING: u64 = 1 << 13;
pub const ACT_RUN_TAB_FORM: u64 = 1 << 14;
pub const ACT_RUN_TAB_MATERIAL: u64 = 1 << 15;
pub const ACT_RUN_TAB_ENV: u64 = 1 << 16;
pub const ACT_RUN_CAM_RESET: u64 = 1 << 17;
pub const ACT_RUN_PIECE_NEXT: u64 = 1 << 18;
pub const ACT_RUN_TOOL_SELECT: u64 = 1 << 19;
pub const ACT_RUN_TOOL_SEW: u64 = 1 << 20;
pub const ACT_RUN_TOOL_MEASURE: u64 = 1 << 21;
pub const ACT_RUN_TOOL_CORNER: u64 = 1 << 22;
pub const ACT_RUN_TOOL_GIZMO: u64 = 1 << 23;
pub const ACT_RUN_TOOL_GRAB: u64 = 1 << 24;
pub const ACT_RUN_TOOL_SEW3D: u64 = 1 << 25;
pub const ACT_RUN_TOOL_PIN: u64 = 1 << 26;
pub const ACT_RUN_TOOL_MEASURE3D: u64 = 1 << 27;
pub const ACT_RUN_TOOL_AVATAR_DIAG: u64 = 1 << 28;
pub const ACT_RUN_TOOL_EDIT2D: u64 = 1 << 29;
pub const ACT_RUN_TOOL_RECT2D: u64 = 1 << 30;
pub const ACT_RUN_TOOL_PEN2D: u64 = 1 << 31;
pub const ACT_RUN_TOOL_FREE2D: u64 = 1 << 32;
pub const ACT_RUN_TOOL_SEW2D: u64 = 1 << 33;
pub const ACT_RUN_TOOL_SPLIT2D: u64 = 1 << 34;
pub const ACT_RUN_TOOL_INTERNAL2D: u64 = 1 << 35;
pub const ACT_RUN_TOOL_MEASURE2D: u64 = 1 << 36;
pub const ACT_RUN_TOOL_TRIM2D: u64 = 1 << 37;
pub const ACT_RUN_TOOL_CORNER2D: u64 = 1 << 38;
pub const ACT_RUN_PIN_MODE_TOGGLE: u64 = 1 << 39;
pub const ACT_RUN_CORNER_MODE_TOGGLE: u64 = 1 << 40;
pub const ACT_RUN_ACCORDION_TOGGLE: u64 = 1 << 41;

pub const ACT_RUN_SOLVER_MENU: u64 = 1 << 42;
pub const ACT_RUN_SOLVER_GPU: u64 = 1 << 43;
pub const ACT_RUN_SOLVER_CPU: u64 = 1 << 44;
pub fn on_runner_solver_menu() {
    dispatch_runner_action(ACT_RUN_SOLVER_MENU);
}
pub fn on_runner_solver_gpu() {
    dispatch_runner_action(ACT_RUN_SOLVER_GPU);
}
pub fn on_runner_solver_cpu() {
    dispatch_runner_action(ACT_RUN_SOLVER_CPU);
}

pub const ACT_RUN_SOLVER_AL: u64 = 1 << 45;
pub fn on_runner_solver_al() {
    dispatch_runner_action(ACT_RUN_SOLVER_AL);
}

static RUNNER_ACTIONS: AtomicU64 = AtomicU64::new(0);

pub fn dispatch_runner_action(act: u64) {
    RUNNER_ACTIONS.fetch_or(act, Ordering::Relaxed);
}

// Trampolines
pub fn on_runner_play_pause() {
    dispatch_runner_action(ACT_RUN_PLAY_PAUSE);
}
pub fn on_runner_step() {
    dispatch_runner_action(ACT_RUN_STEP);
}
pub fn on_runner_reset() {
    dispatch_runner_action(ACT_RUN_RESET);
}
pub fn on_runner_save() {
    dispatch_runner_action(ACT_RUN_SAVE);
}
pub fn on_runner_export() {
    dispatch_runner_action(ACT_RUN_EXPORT);
}
pub fn on_runner_clean_view() {
    dispatch_runner_action(ACT_RUN_CLEAN_VIEW);
}
pub fn on_runner_palette_toggle() {
    dispatch_runner_action(ACT_RUN_PALETTE_TOGGLE);
}
pub fn on_runner_theme_toggle() {
    dispatch_runner_action(ACT_RUN_THEME_TOGGLE);
}
pub fn on_runner_view_split() {
    dispatch_runner_action(ACT_RUN_VIEW_SPLIT);
}
pub fn on_runner_view_3d() {
    dispatch_runner_action(ACT_RUN_VIEW_3D);
}
pub fn on_runner_view_2d() {
    dispatch_runner_action(ACT_RUN_VIEW_2D);
}
pub fn on_runner_drawer_toggle() {
    dispatch_runner_action(ACT_RUN_DRAWER_TOGGLE);
}
pub fn on_runner_tab_pattern() {
    dispatch_runner_action(ACT_RUN_TAB_PATTERN);
}
pub fn on_runner_tab_sewing() {
    dispatch_runner_action(ACT_RUN_TAB_SEWING);
}
pub fn on_runner_tab_form() {
    dispatch_runner_action(ACT_RUN_TAB_FORM);
}
pub fn on_runner_tab_material() {
    dispatch_runner_action(ACT_RUN_TAB_MATERIAL);
}
pub fn on_runner_tab_env() {
    dispatch_runner_action(ACT_RUN_TAB_ENV);
}
pub fn on_runner_cam_reset() {
    dispatch_runner_action(ACT_RUN_CAM_RESET);
}
pub fn on_runner_piece_next() {
    dispatch_runner_action(ACT_RUN_PIECE_NEXT);
}
pub fn on_runner_tool_select() {
    dispatch_runner_action(ACT_RUN_TOOL_SELECT);
}
pub fn on_runner_tool_sew() {
    dispatch_runner_action(ACT_RUN_TOOL_SEW);
}
pub fn on_runner_tool_measure() {
    dispatch_runner_action(ACT_RUN_TOOL_MEASURE);
}
pub fn on_runner_tool_corner() {
    dispatch_runner_action(ACT_RUN_TOOL_CORNER);
}
pub fn on_runner_tool_gizmo() {
    dispatch_runner_action(ACT_RUN_TOOL_GIZMO);
}
pub fn on_runner_tool_grab() {
    dispatch_runner_action(ACT_RUN_TOOL_GRAB);
}
pub fn on_runner_tool_sew3d() {
    dispatch_runner_action(ACT_RUN_TOOL_SEW3D);
}
pub fn on_runner_tool_pin() {
    dispatch_runner_action(ACT_RUN_TOOL_PIN);
}
pub fn on_runner_tool_measure3d() {
    dispatch_runner_action(ACT_RUN_TOOL_MEASURE3D);
}
pub fn on_runner_tool_avatar_diag() {
    dispatch_runner_action(ACT_RUN_TOOL_AVATAR_DIAG);
}
pub fn on_runner_2d_edit() {
    dispatch_runner_action(ACT_RUN_TOOL_EDIT2D);
}
pub fn on_runner_2d_rect() {
    dispatch_runner_action(ACT_RUN_TOOL_RECT2D);
}
pub fn on_runner_2d_pen() {
    dispatch_runner_action(ACT_RUN_TOOL_PEN2D);
}
pub fn on_runner_2d_freehand() {
    dispatch_runner_action(ACT_RUN_TOOL_FREE2D);
}
pub fn on_runner_2d_sew() {
    dispatch_runner_action(ACT_RUN_TOOL_SEW2D);
}
pub fn on_runner_2d_split() {
    dispatch_runner_action(ACT_RUN_TOOL_SPLIT2D);
}
pub fn on_runner_2d_internal() {
    dispatch_runner_action(ACT_RUN_TOOL_INTERNAL2D);
}
pub fn on_runner_2d_measure() {
    dispatch_runner_action(ACT_RUN_TOOL_MEASURE2D);
}
pub fn on_runner_2d_trim() {
    dispatch_runner_action(ACT_RUN_TOOL_TRIM2D);
}
pub fn on_runner_2d_corner() {
    dispatch_runner_action(ACT_RUN_TOOL_CORNER2D);
}
pub fn on_runner_pin_mode_toggle() {
    dispatch_runner_action(ACT_RUN_PIN_MODE_TOGGLE);
}
pub fn on_runner_corner_mode_toggle() {
    dispatch_runner_action(ACT_RUN_CORNER_MODE_TOGGLE);
}
pub fn on_runner_accordion_toggle() {
    dispatch_runner_action(ACT_RUN_ACCORDION_TOGGLE);
}

// ============================================================================
// Object Browser Category Enum
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ObjectCategory {
    #[default]
    Pattern2D,
    Sewing,
    Form3D,
    Material,
    Environment,
}

// ============================================================================
// State Struct
// ============================================================================

pub struct CssRunnerState {
    pub smoke: bool,
    pub smoke_done: std::rc::Rc<std::cell::Cell<bool>>,
    pub usd_scene: Option<cssimpler::renderer::gpu::usd::UsdScene>,
    pub project: FittingProject,
    pub scene: FittingScene,
    pub initial_scene: FittingScene,
    pub files: DocumentFiles,
    pub settings: VbdSettings,
    pub worker_tx: mpsc::Sender<Work>,
    pub worker_latest: Latest,
    pub generation: u64,
    pub worker_pending: bool,
    pub playing: bool,
    pub step_count: u64,
    pub step_ms: f64,
    pub last_metrics: Option<VbdMetrics>,
    pub camera: Camera,
    pub theme: Theme,
    pub layout_mode: ViewLayoutMode,
    pub active_category: ObjectCategory,
    pub active_3d_tool: String,
    pub active_2d_tool: String,
    pub clean_view: bool,
    pub command_palette_open: bool,
    pub left_drawer_open: bool,
    pub selected_piece_index: usize,
    pub status_message: String,
    pub pointer_down: bool,
    pub last_pointer_pos: (f32, f32),
    pub gpu_sim: bool,
    pub contact_method: ContactMethod,
    pub solver_menu_open: bool,
    pub drag_test: bool,
    pub perf_test: bool,
    pub authoring_test: bool,
    pub perf: Option<super::perf::PerfRun>,
    pub cadence: super::perf::Cadence,
    pub should_exit: bool,
    pub pin_mode_contour: bool,
    pub corner_mode_radius: bool,
    pub open_accordion_idx: usize,
}

impl CssRunnerState {
    pub fn new(
        project: FittingProject,
        scene: FittingScene,
        files: DocumentFiles,
        worker_tx: mpsc::Sender<Work>,
        worker_latest: Latest,
    ) -> Self {
        let settings = project.simulation.clone().unwrap_or_default();
        let initial_scene = scene.clone();

        Self {
            smoke: false,
            smoke_done: Default::default(),
            usd_scene: None,
            project,
            scene,
            initial_scene,
            files,
            settings,
            worker_tx,
            worker_latest,
            generation: 1,
            worker_pending: false,
            playing: false,
            step_count: 0,
            step_ms: 0.0,
            last_metrics: None,
            camera: Camera::default(),
            theme: Theme::Dark,
            layout_mode: ViewLayoutMode::Split,
            active_category: ObjectCategory::Pattern2D,
            active_3d_tool: "Gizmo".to_string(),
            active_2d_tool: "Edit".to_string(),
            clean_view: false,
            command_palette_open: false,
            left_drawer_open: true,
            selected_piece_index: 0,
            status_message: "Ready".to_string(),
            pointer_down: false,
            last_pointer_pos: (0.0, 0.0),
            gpu_sim: false,
            contact_method: ContactMethod::Ipc,
            solver_menu_open: false,
            drag_test: false,
            perf_test: false,
            authoring_test: false,
            perf: None,
            cadence: Default::default(),
            should_exit: false,
            pin_mode_contour: true,
            corner_mode_radius: true,
            open_accordion_idx: 0,
        }
    }

    pub fn send_control_state(&mut self, once: bool) {
        let work = Work {
            generation: self.generation,
            scene: self.scene.clone(),
            settings: self.settings.clone(),
            grab: None,
            gpu: self.gpu_sim,
            contact_method: self.contact_method,
            signal: worker::RepaintSignal::noop(),
            playing: self.playing,
            once,
        };
        if self.worker_tx.send(work).is_ok() {
            self.worker_pending = true;
        }
    }

    pub fn trigger_worker_step(&mut self, once: bool) {
        self.send_control_state(once);
    }

    pub fn sync_usd_points(&mut self) {
        if let Some(scene) = &self.usd_scene {
            for panel in &self.scene.garment.panels {
                if let Err(error) = marvelous_cloth::usd_scene::update_viewport_points(
                    scene,
                    &marvelous_cloth::usd_scene::viewport_panel_path(&panel.id),
                    &self.scene.state.x[panel.vertices.clone()],
                ) {
                    self.status_message = format!("USD update error: {error}");
                }
            }
        }
    }

    /// Transfer the last displayed accepted x/v/time; discard in-flight results
    /// by generation. Playback pauses; rest geometry and document history stay intact.
    pub fn select_solver(&mut self, gpu: bool) {
        self.select_solver_method(gpu, ContactMethod::Ipc);
    }
    pub fn select_solver_method(&mut self, gpu: bool, method: ContactMethod) {
        self.solver_menu_open = false;
        if self.gpu_sim == gpu && self.contact_method == method {
            return;
        }
        self.gpu_sim = gpu;
        self.contact_method = method;
        self.generation = self.generation.wrapping_add(1);
        self.playing = false;
        self.worker_pending = false;
        self.last_metrics = None;
        self.step_ms = 0.0;
        self.status_message =
            "Solver changed; paused at displayed state. Press Play or Step.".into();
        self.send_control_state(false);
    }

    pub fn reset_simulation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.scene = self.initial_scene.clone();
        self.step_count = 0;
        self.step_ms = 0.0;
        self.last_metrics = None;
        self.worker_pending = false;
        self.playing = false;
        self.status_message = "Reset simulation state".to_string();
        self.sync_usd_points();
        self.send_control_state(false);
    }

    pub fn save_project(&mut self) {
        let _ = fs::create_dir_all(&self.files.output);
        let path = self.files.output.join("fitting-project.json");
        match self.project.save(&path) {
            Ok(_) => self.status_message = format!("Saved {}", path.display()),
            Err(e) => self.status_message = format!("Save error: {e}"),
        }
    }

    pub fn export_artifacts(&mut self) {
        let _ = fs::create_dir_all(&self.files.output);
        match export::write_artifacts(
            &self.project.template,
            &self.scene.garment,
            &self.scene.state,
            &self.files.output,
        ) {
            Ok(_) => {
                self.status_message =
                    format!("Exported artifacts to {}", self.files.output.display())
            }
            Err(e) => self.status_message = format!("Export error: {e}"),
        }
    }
}

// ============================================================================
// Update and Event Handling
// ============================================================================

pub fn update_runner(state: &mut CssRunnerState, frame: FrameInfo) -> Invalidation {
    // 1. Poll simulation worker result
    let done = state
        .worker_latest
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    let mut had_result = false;
    if let Some(done) = done
        && done.generation == state.generation
    {
        state.worker_pending = false;
        match done.result {
            Ok((new_state, metrics)) => {
                had_result = true;
                state.scene.state = new_state;
                state.step_count += 1;
                state.step_ms = metrics.step_ms;
                let residual = metrics.barrier_al.as_ref().map_or_else(
                    || {
                        metrics
                            .residual_rms_n
                            .map_or("N/A".into(), |r| format!("{r:.1e} N RMS"))
                    },
                    |al| {
                        format!(
                            "{:.1e} N max, gap {:.1e} m",
                            al.original_force_max_n, al.primal_m
                        )
                    },
                );
                state.status_message = format!(
                    "Step {}: min dist {:.3} mm, residual {}, {:.1} ms",
                    state.step_count,
                    metrics.minimum_distance_m * 1000.,
                    residual,
                    metrics.step_ms
                );
                state.last_metrics = Some(metrics);
                state.sync_usd_points();
            }
            Err(e) => {
                state.playing = false;
                state.status_message = format!("Worker error: {e}");
                state.send_control_state(false);
            }
        }
    }

    state.cadence.update(
        std::time::Instant::now(),
        state.scene.state.time,
        state.generation,
        state.playing,
    );

    if let Some(perf) = state.perf.as_mut() {
        let dt = state.settings.dt;
        if let Some(report) = perf.tick(state.scene.state.time, frame.frame_index, dt, true) {
            let _ = fs::create_dir_all(&state.files.output);
            let perf_path = state.files.output.join("ui-perf.json");
            let _ = fs::write(
                &perf_path,
                serde_json::to_string_pretty(&report).unwrap_or_default(),
            );
            state.should_exit = true;
        } else if perf.expired() {
            state.should_exit = true;
        }
    }

    // 2. Process pending UI actions
    let actions = RUNNER_ACTIONS.swap(0, Ordering::Relaxed);

    if actions & ACT_RUN_SOLVER_MENU != 0 {
        state.solver_menu_open = !state.solver_menu_open;
    }
    if actions & ACT_RUN_SOLVER_GPU != 0 {
        state.select_solver(true);
    }
    if actions & ACT_RUN_SOLVER_AL != 0 {
        state.select_solver_method(true, ContactMethod::BarrierAl);
    }
    if actions & ACT_RUN_SOLVER_CPU != 0 {
        state.select_solver(false);
    }
    if actions & ACT_RUN_PLAY_PAUSE != 0 {
        state.playing = !state.playing;
        state.send_control_state(false);
    }
    if actions & ACT_RUN_STEP != 0 {
        state.send_control_state(true);
    }
    if actions & ACT_RUN_RESET != 0 {
        state.reset_simulation();
    }
    if actions & ACT_RUN_SAVE != 0 {
        state.save_project();
    }
    if actions & ACT_RUN_EXPORT != 0 {
        state.export_artifacts();
    }
    if actions & ACT_RUN_CLEAN_VIEW != 0 {
        state.clean_view = !state.clean_view;
    }
    if actions & ACT_RUN_PALETTE_TOGGLE != 0 {
        state.command_palette_open = !state.command_palette_open;
    }
    if actions & ACT_RUN_THEME_TOGGLE != 0 {
        state.theme = state.theme.toggled();
    }
    if actions & ACT_RUN_VIEW_SPLIT != 0 {
        state.layout_mode = ViewLayoutMode::Split;
    }
    if actions & ACT_RUN_VIEW_3D != 0 {
        state.layout_mode = ViewLayoutMode::ThreeDOnly;
    }
    if actions & ACT_RUN_VIEW_2D != 0 {
        state.layout_mode = ViewLayoutMode::TwoDOnly;
    }
    if actions & ACT_RUN_DRAWER_TOGGLE != 0 {
        state.left_drawer_open = !state.left_drawer_open;
    }
    if actions & ACT_RUN_TAB_PATTERN != 0 {
        state.active_category = ObjectCategory::Pattern2D;
    }
    if actions & ACT_RUN_TAB_SEWING != 0 {
        state.active_category = ObjectCategory::Sewing;
    }
    if actions & ACT_RUN_TAB_FORM != 0 {
        state.active_category = ObjectCategory::Form3D;
    }
    if actions & ACT_RUN_TAB_MATERIAL != 0 {
        state.active_category = ObjectCategory::Material;
    }
    if actions & ACT_RUN_TAB_ENV != 0 {
        state.active_category = ObjectCategory::Environment;
    }
    if actions & ACT_RUN_CAM_RESET != 0 {
        state.camera = Camera::default();
    }
    if actions & ACT_RUN_PIECE_NEXT != 0 {
        let count = state.project.template.panels.len().max(1);
        state.selected_piece_index = (state.selected_piece_index + 1) % count;
    }
    if actions & ACT_RUN_TOOL_GIZMO != 0 || actions & ACT_RUN_TOOL_SELECT != 0 {
        state.active_3d_tool = "Gizmo".to_string();
    }
    if actions & ACT_RUN_TOOL_GRAB != 0 {
        state.active_3d_tool = "Grab".to_string();
    }
    if actions & ACT_RUN_TOOL_SEW3D != 0 {
        state.active_3d_tool = "3D Sew".to_string();
    }
    if actions & ACT_RUN_TOOL_PIN != 0 {
        state.active_3d_tool = "Pin".to_string();
    }
    if actions & ACT_RUN_TOOL_MEASURE3D != 0 {
        state.active_3d_tool = "Measure".to_string();
    }
    if actions & ACT_RUN_TOOL_AVATAR_DIAG != 0 {
        state.active_3d_tool = "Avatar Diagnostics".to_string();
    }
    if actions & ACT_RUN_TOOL_EDIT2D != 0 {
        state.active_2d_tool = "Edit".to_string();
    }
    if actions & ACT_RUN_TOOL_RECT2D != 0 {
        state.active_2d_tool = "Rect".to_string();
    }
    if actions & ACT_RUN_TOOL_PEN2D != 0 {
        state.active_2d_tool = "Pen".to_string();
    }
    if actions & ACT_RUN_TOOL_FREE2D != 0 {
        state.active_2d_tool = "Freehand".to_string();
    }
    if actions & ACT_RUN_TOOL_SEW2D != 0 || actions & ACT_RUN_TOOL_SEW != 0 {
        state.active_2d_tool = "Sew".to_string();
    }
    if actions & ACT_RUN_TOOL_SPLIT2D != 0 {
        state.active_2d_tool = "Split".to_string();
    }
    if actions & ACT_RUN_TOOL_INTERNAL2D != 0 {
        state.active_2d_tool = "Internal Line".to_string();
    }
    if actions & ACT_RUN_TOOL_MEASURE2D != 0 || actions & ACT_RUN_TOOL_MEASURE != 0 {
        state.active_2d_tool = "Measure".to_string();
    }
    if actions & ACT_RUN_TOOL_TRIM2D != 0 {
        state.active_2d_tool = "Trim".to_string();
    }
    if actions & ACT_RUN_TOOL_CORNER2D != 0 || actions & ACT_RUN_TOOL_CORNER != 0 {
        state.active_2d_tool = "Corner".to_string();
    }
    if actions & ACT_RUN_PIN_MODE_TOGGLE != 0 {
        state.pin_mode_contour = !state.pin_mode_contour;
    }
    if actions & ACT_RUN_CORNER_MODE_TOGGLE != 0 {
        state.corner_mode_radius = !state.corner_mode_radius;
    }
    if actions & ACT_RUN_ACCORDION_TOGGLE != 0 {
        state.open_accordion_idx = (state.open_accordion_idx + 1) % 9;
    }

    if state.playing {
        if had_result {
            Invalidation::Paint
        } else {
            Invalidation::Clean
        }
    } else if actions != 0 || had_result {
        Invalidation::Structure
    } else {
        Invalidation::Clean
    }
}

pub fn handle_runner_event(state: &mut CssRunnerState, event: &EngineEvent) -> bool {
    match event {
        EngineEvent::Key(k) => {
            if k.state == cssimpler::renderer::ButtonState::Pressed {
                if let cssimpler::renderer::KeyIdentity::Character(ref ch) = k.logical_key {
                    if ch == " " {
                        state.playing = !state.playing;
                        state.send_control_state(false);
                        return true;
                    }
                    if ch == "k" && (k.modifiers.control || k.modifiers.super_key) {
                        state.command_palette_open = !state.command_palette_open;
                        return true;
                    }
                    if ch == "s" && (k.modifiers.control || k.modifiers.super_key) {
                        state.save_project();
                        return true;
                    }
                }
                if let cssimpler::renderer::KeyIdentity::Named(ref name) = k.logical_key
                    && name == "Escape"
                    && (state.command_palette_open || state.solver_menu_open)
                {
                    state.command_palette_open = false;
                    state.solver_menu_open = false;
                    return true;
                }
            }
            false
        }
        EngineEvent::PointerButton {
            button: cssimpler::renderer::PointerButton::Primary,
            state: btn_state,
            ..
        } => {
            state.pointer_down = *btn_state == cssimpler::renderer::ButtonState::Pressed;
            false
        }
        EngineEvent::PointerMoved { position, .. } => {
            let (px, py) = (position.x, position.y);
            if state.pointer_down {
                let dx = px - state.last_pointer_pos.0;
                let dy = py - state.last_pointer_pos.1;
                state.camera.yaw += dx * 0.008;
                state.camera.pitch = (state.camera.pitch + dy * 0.008).clamp(-1.45, 1.45);
            }
            state.last_pointer_pos = (px, py);
            false
        }
        EngineEvent::Scroll { delta, .. } => {
            let dy = match delta {
                cssimpler::renderer::ScrollDelta::Lines { y, .. } => *y * 0.2,
                cssimpler::renderer::ScrollDelta::Pixels { y, .. } => *y * 0.005,
            };
            state.camera.distance = (state.camera.distance - dy).clamp(0.5, 15.0);
            false
        }
        _ => false,
    }
}

// ============================================================================
// UI Generation (Studio V2 layout via cssimpler::ui!)
// ============================================================================

pub fn build_runner_ui(state: &CssRunnerState) -> Node {
    let theme_class = state.theme.class_name();
    let shell_class = format!("studio-shell {theme_class}");

    ui! {
        <div id="app" class={shell_class}>
            {build_runner_top_bar(state)}
            <div class="studio-body">
                {build_runner_3d_rail(state)}
                {build_runner_left_browser(state)}
                {build_runner_stage(state)}
                {build_runner_2d_rail(state)}
                {build_runner_right_dock(state)}
            </div>
            {build_runner_status_bar(state)}
            {build_runner_command_palette(state)}
        </div>
    }
}

fn build_runner_top_bar(state: &CssRunnerState) -> Node {
    let play_label = if state.playing { "Pause" } else { "Play" };
    let play_class = if state.playing {
        "transport-btn playing"
    } else {
        "transport-btn"
    };

    let (split_class, threed_class, twod_class) = match state.layout_mode {
        ViewLayoutMode::Split => ("layout-btn active", "layout-btn", "layout-btn"),
        ViewLayoutMode::ThreeDOnly => ("layout-btn", "layout-btn active", "layout-btn"),
        ViewLayoutMode::TwoDOnly => ("layout-btn", "layout-btn", "layout-btn active"),
    };

    let clean_class = if state.clean_view {
        "clean-view-toggle active"
    } else {
        "clean-view-toggle"
    };

    let (backend_class, backend_label) = if state.gpu_sim {
        ("backend-badge gpu", state.contact_method.label())
    } else {
        ("backend-badge cpu", "CPU (OGC)")
    };

    let theme_label = match state.theme {
        Theme::Dark => "Theme: Dark",
        Theme::Light => "Theme: Light",
    };

    ui! {
        <header id="top-bar" class="top-bar">
            <div class="top-bar-left">
                <span class="app-title">"FoldCraft | Simulation Studio V2"</span>
                <nav class="top-bar-menus">
                    <button class="menu-item" onclick={on_runner_save}>"Save Project (Ctrl+S)"</button>
                    <button class="menu-item" onclick={on_runner_export}>"Export Artifacts"</button>
                </nav>
            </div>
            <div class="transport-controls">
                <button id="sim-play-btn" class={play_class} onclick={on_runner_play_pause} title="Start/Stop simulation (Space)">
                    {if state.playing { icon_pause() } else { icon_play() }}
                    <span class="btn-label">{play_label}</span>
                </button>
                <button id="sim-step-btn" class="transport-btn" onclick={on_runner_step} title="Advance single solver step">
                    {icon_step()}
                    <span class="btn-label">"Step"</span>
                </button>
                <button id="sim-reset-btn" class="transport-btn" onclick={on_runner_reset} title="Reset simulation state">
                    {icon_reset()}
                    <span class="btn-label">"Reset"</span>
                </button>
                <div class="solver-picker">
                    <button id="sim-backend-indicator" class={backend_class} onclick={on_runner_solver_menu} title="Choose active solver">{format!("{backend_label} ▾")}</button>
                    {if state.solver_menu_open { ui! {
                        <div id="solver-dropdown" class="solver-dropdown">
                            <button id="solver-wgsl-ipc" class="menu-item" onclick={on_runner_solver_gpu}>"WGSL AVBD / IPC"</button>
                            <button id="solver-barrier-al" class="menu-item" onclick={on_runner_solver_al}>"WGSL AVBD / barrier AL (experimental)"</button>
                            <button id="solver-cpu" class="menu-item" onclick={on_runner_solver_cpu}>"CPU VBD / OGC"</button>
                        </div>
                    }} else { ui! { <span /> } }}
                </div>
            </div>
            <div class="top-bar-right">
                <div class="layout-switcher">
                    <button class={split_class} onclick={on_runner_view_split} title="Split 3D + 2D View">
                        {icon_view_split()}
                        <span class="btn-label">"Split"</span>
                    </button>
                    <button class={threed_class} onclick={on_runner_view_3d} title="Full 3D Garment View">
                        {icon_view_3d()}
                        <span class="btn-label">"3D"</span>
                    </button>
                    <button class={twod_class} onclick={on_runner_view_2d} title="Full 2D Pattern Board">
                        {icon_view_2d()}
                        <span class="btn-label">"2D"</span>
                    </button>
                </div>
                <button id="clean-view-toggle" class={clean_class} onclick={on_runner_clean_view} title="Minimalist Presentation View">
                    {icon_clean_view()}
                    <span class="btn-label">"Clean View"</span>
                </button>
                <button id="command-palette-trigger" onclick={on_runner_palette_toggle} title="Open Command Palette (Ctrl+K)">
                    {icon_palette()}
                    <span class="btn-label">"Palette"</span>
                </button>
                <button id="theme-toggle" onclick={on_runner_theme_toggle} title="Toggle Dark/Light Theme">
                    {match state.theme {
                        Theme::Dark => icon_sun(),
                        Theme::Light => icon_moon(),
                    }}
                    <span class="btn-label">{theme_label}</span>
                </button>
            </div>
        </header>
    }
}

fn build_runner_3d_rail(state: &CssRunnerState) -> Node {
    let gizmo_class = if state.active_3d_tool == "Gizmo" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let grab_class = if state.active_3d_tool == "Grab" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let sew3d_class = if state.active_3d_tool == "3D Sew" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let pin_class = if state.active_3d_tool == "Pin" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let msr_class = if state.active_3d_tool == "Measure" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let avatar_class = if state.active_3d_tool == "Avatar Diagnostics" {
        "tool-btn active"
    } else {
        "tool-btn"
    };

    let pin_badge = if state.pin_mode_contour {
        "Contour"
    } else {
        "Interior"
    };

    ui! {
        <aside id="rail-3d" class="tool-rail tool-rail-3d">
            <button class={gizmo_class} onclick={on_runner_tool_gizmo} title="Select & Arrange (Gizmo)">
                {icon_gizmo()}
                <span class="tool-label">"Gizmo"</span>
            </button>
            <div class="tool-composite grab-composite">
                <button class={grab_class} onclick={on_runner_tool_grab} title="Grab Cloth (Pull & Drag)">
                    {icon_grab()}
                    <span class="tool-label">"Grab"</span>
                </button>
                <div class="grab-readouts">
                    <span class="readout">"50mm"</span>
                    <span class="readout">"1.0"</span>
                </div>
            </div>
            <button class={sew3d_class} onclick={on_runner_tool_sew3d} title="3D Sew (Connect boundaries)">
                {icon_sew_3d()}
                <span class="tool-label">"3D Sew"</span>
            </button>
            <div class="tool-composite pin-composite">
                <button class={pin_class} onclick={on_runner_tool_pin} title="Pin Cloth">
                    {icon_pin()}
                    <span class="tool-label">"Pin"</span>
                </button>
                <button class="pin-mode-badge" onclick={on_runner_pin_mode_toggle} title="Toggle Contour / Interior pin mode">
                    {pin_badge}
                </button>
            </div>
            <button class={msr_class} onclick={on_runner_tool_measure3d} title="3D Distance Measurement">
                {icon_measure_3d()}
                <span class="tool-label">"Measure"</span>
            </button>
            <button id="avatar-diagnostics-btn" class={avatar_class} onclick={on_runner_tool_avatar_diag} title="Avatar Collision Diagnostics">
                {icon_avatar_diag()}
                <span class="tool-label">"Avatar"</span>
            </button>
            <button class="tool-btn" onclick={on_runner_cam_reset} title="Reset Camera View">
                {icon_reset()}
                <span class="tool-label">"Reset"</span>
            </button>
            <button class="tool-btn" onclick={on_runner_drawer_toggle} title="Toggle Asset Drawer">
                {icon_view_split()}
                <span class="tool-label">"Drawer"</span>
            </button>
        </aside>
    }
}

fn build_runner_2d_rail(state: &CssRunnerState) -> Node {
    if state.clean_view {
        return ui! { <aside class="tool-rail tool-rail-2d" style="display:none"></aside> };
    }

    let edit_class = if state.active_2d_tool == "Edit" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let rect_class = if state.active_2d_tool == "Rect" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let pen_class = if state.active_2d_tool == "Pen" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let free_class = if state.active_2d_tool == "Freehand" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let sew_class = if state.active_2d_tool == "Sew" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let split_class = if state.active_2d_tool == "Split" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let line_class = if state.active_2d_tool == "Internal Line" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let tape_class = if state.active_2d_tool == "Measure" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let trim_class = if state.active_2d_tool == "Trim" {
        "tool-btn active"
    } else {
        "tool-btn"
    };
    let corner_class = if state.active_2d_tool == "Corner" {
        "tool-btn active"
    } else {
        "tool-btn"
    };

    let corner_badge = if state.corner_mode_radius {
        "Rad"
    } else {
        "Shape"
    };

    ui! {
        <aside id="rail-2d" class="tool-rail tool-rail-2d">
            <button class={edit_class} onclick={on_runner_2d_edit} title="Select & Edit Vertices">
                {icon_edit_2d()}
                <span class="tool-label">"Edit"</span>
            </button>
            <button class={rect_class} onclick={on_runner_2d_rect} title="Create Rectangle Pattern">
                {icon_rect_2d()}
                <span class="tool-label">"Rect"</span>
            </button>
            <button class={pen_class} onclick={on_runner_2d_pen} title="Draw Polyline Edge">
                {icon_pen_2d()}
                <span class="tool-label">"Pen"</span>
            </button>
            <button class={free_class} onclick={on_runner_2d_freehand} title="Freehand Curve">
                {icon_freehand_2d()}
                <span class="tool-label">"Free"</span>
            </button>
            <button class={sew_class} onclick={on_runner_2d_sew} title="2D Seam Correspondence">
                {icon_sew_2d()}
                <span class="tool-label">"Sew"</span>
            </button>
            <button class={split_class} onclick={on_runner_2d_split} title="Split Boundary Point">
                {icon_split_2d()}
                <span class="tool-label">"Split"</span>
            </button>
            <button class={line_class} onclick={on_runner_2d_internal} title="Internal Line / Dart">
                {icon_internal_2d()}
                <span class="tool-label">"Line"</span>
            </button>
            <button class={tape_class} onclick={on_runner_2d_measure} title="Tape Measure">
                {icon_measure_2d()}
                <span class="tool-label">"Tape"</span>
            </button>
            <button class={trim_class} onclick={on_runner_2d_trim} title="Construction Trim">
                {icon_trim_2d()}
                <span class="tool-label">"Trim"</span>
            </button>
            <div class="tool-composite">
                <button class={corner_class} onclick={on_runner_2d_corner} title="Corner Rounding Tool">
                    {icon_corner_2d()}
                    <span class="tool-label">"Corner"</span>
                </button>
                <button class="corner-mode-badge" onclick={on_runner_corner_mode_toggle} title="Toggle Radius / Shape corner rule">
                    {corner_badge}
                </button>
            </div>
        </aside>
    }
}

fn build_runner_left_browser(state: &CssRunnerState) -> Node {
    if !state.left_drawer_open || state.clean_view {
        return ui! { <aside class="left-drawer collapsed"></aside> };
    }

    let panels = &state.project.template.panels;
    let mut piece_cards = Vec::new();

    for (idx, p) in panels.iter().enumerate() {
        let is_selected = idx == state.selected_piece_index;
        let card_class = if is_selected {
            "piece-card active"
        } else {
            "piece-card"
        };
        let label = p.name.clone();
        let id_str = p.id.clone();
        let meta_str = format!("Fabric: {} · Grain: {:.0}°", p.fabric, p.grain_degrees);
        piece_cards.push(ui! {
            <div class={card_class}>
                <div class="piece-header">
                    <span class="piece-name">{label}</span>
                    <span class="piece-id">{id_str}</span>
                </div>
                <div class="piece-meta">
                    <span>{meta_str}</span>
                </div>
            </div>
        });
    }

    ui! {
        <aside id="left-browser" class="sidebar left-browser-panel">
            <header class="browser-header">
                <h2 class="browser-title">"Assets & Pieces"</h2>
                <span class="browser-badge">{format!("{} pieces", panels.len())}</span>
            </header>
            <div class="drawer-content">
                <div class="browser-section">
                    <div class="section-header">
                        <span class="section-title">"Bundled Templates"</span>
                        <span class="section-subtitle">"Engineering fixtures (metres)"</span>
                    </div>
                    <div class="template-cards-container">
                        <div class="template-entry">
                            <div class="template-header">
                                <span class="template-title">"T-shirt"</span>
                                <span class="template-meta">"2 panels · 4 seams"</span>
                            </div>
                            <span class="template-description">"Two-panel engineering fixture for MakeHuman mannequin."</span>
                        </div>
                        <div class="template-entry">
                            <div class="template-header">
                                <span class="template-title">"Dress"</span>
                                <span class="template-meta">"2 panels · 4 seams"</span>
                            </div>
                            <span class="template-description">"Two-panel dress fixture with curved seams."</span>
                        </div>
                        <div class="template-entry">
                            <div class="template-header">
                                <span class="template-title">"Apron"</span>
                                <span class="template-meta">"3 panels · 4 seams"</span>
                            </div>
                            <span class="template-description">"Workwear fixture with ties and bib."</span>
                        </div>
                    </div>
                </div>

                <div class="browser-section">
                    <div class="section-header">
                        <span class="section-title">"Import & Export"</span>
                    </div>
                    <div class="quick-action-group">
                        <span class="group-label">"Import"</span>
                        <div class="quick-actions-row">
                            <button class="btn-action">"DXF"</button>
                            <button class="btn-action">"SVG"</button>
                            <button class="btn-action">"Avatar"</button>
                        </div>
                    </div>
                    <div class="quick-action-group">
                        <span class="group-label">"Export"</span>
                        <div class="quick-actions-row">
                            <button class="btn-action" onclick={on_runner_export}>"Export OBJ"</button>
                            <button class="btn-action btn-action-accent" onclick={on_runner_export}>"True-Scale SVG"</button>
                        </div>
                        <div class="export-accuracy-note-box">
                            <p class="export-note-text">
                                "Physical SVG includes stored seam allowances & sewing notches. 100 mm reference ruler verified."
                            </p>
                        </div>
                    </div>
                </div>

                <div class="piece-cards-section">
                    <div class="section-header">
                        <span class="section-title">"Garment Pieces"</span>
                    </div>
                    <div class="piece-cards-list">
                        {piece_cards}
                    </div>
                    <button class="action-btn" onclick={on_runner_piece_next}>"Select Next Piece"</button>
                </div>
            </div>
        </aside>
    }
}

fn build_runner_stage(state: &CssRunnerState) -> Node {
    let stage_class = format!("stage {}", state.layout_mode.stage_class());

    ui! {
        <main id="viewport-stage" class={stage_class}>
            {build_runner_3d_viewport(state)}
            {build_runner_2d_viewport(state)}
        </main>
    }
}

fn build_runner_3d_viewport(_state: &CssRunnerState) -> Node {
    ui! {
        <div id="viewport-3d" class="viewport-container">
            <div class="viewport-header">
                <span class="viewport-title">"3D Garment & Avatar"</span>
                <span class="viewport-sub">"USD native 3D viewport (VBD + OGC)"</span>
            </div>
            <div class="canvas-area">
                <div id="runner-viewport-3d-scene" class="viewport"></div>
            </div>
        </div>
    }
}

fn build_runner_2d_viewport(state: &CssRunnerState) -> Node {
    let (svg_nodes, view_box) = render_runner_2d_svg(state);

    ui! {
        <div id="viewport-2d" class="viewport-container">
            <div class="viewport-header">
                <span class="viewport-title">"2D Pattern Board"</span>
                <span class="viewport-sub">"Metric rest geometry (metres)"</span>
            </div>
            <div class="canvas-area">
                <svg viewBox={view_box} class="svg-canvas">
                    {svg_nodes}
                </svg>
            </div>
        </div>
    }
}

fn build_runner_right_dock(state: &CssRunnerState) -> Node {
    if state.clean_view {
        return ui! { <aside class="right-dock collapsed"></aside> };
    }

    let (tp, ts, tf, tm, te) = match state.active_category {
        ObjectCategory::Pattern2D => (
            "dock-tab active",
            "dock-tab",
            "dock-tab",
            "dock-tab",
            "dock-tab",
        ),
        ObjectCategory::Sewing => (
            "dock-tab",
            "dock-tab active",
            "dock-tab",
            "dock-tab",
            "dock-tab",
        ),
        ObjectCategory::Form3D => (
            "dock-tab",
            "dock-tab",
            "dock-tab active",
            "dock-tab",
            "dock-tab",
        ),
        ObjectCategory::Material => (
            "dock-tab",
            "dock-tab",
            "dock-tab",
            "dock-tab active",
            "dock-tab",
        ),
        ObjectCategory::Environment => (
            "dock-tab",
            "dock-tab",
            "dock-tab",
            "dock-tab",
            "dock-tab active",
        ),
    };

    let content = match state.active_category {
        ObjectCategory::Pattern2D => build_cat_pattern_2d(state),
        ObjectCategory::Sewing => build_cat_sewing(state),
        ObjectCategory::Form3D => build_cat_form_3d(state),
        ObjectCategory::Material => build_cat_material(state),
        ObjectCategory::Environment => build_cat_environment(state),
    };

    ui! {
        <aside id="right-dock" class="sidebar right-object-browser-panel">
            <div id="object-browser">
                <div class="dock-section-title">"Object Browser"</div>
                <div class="dock-tabs">
                    <button class={tp} onclick={on_runner_tab_pattern}>"2D Pattern"</button>
                    <button class={ts} onclick={on_runner_tab_sewing}>"Sewing"</button>
                    <button class={tf} onclick={on_runner_tab_form}>"3D Form"</button>
                    <button class={tm} onclick={on_runner_tab_material}>"Material"</button>
                    <button class={te} onclick={on_runner_tab_env}>"Environment"</button>
                </div>
                <div class="object-browser-content-viewport">
                    {content}
                </div>
            </div>
            <div id="property-editor">
                <div class="dock-section-title">"Property Editor"</div>
                {build_runner_property_accordions(state)}
            </div>
        </aside>
    }
}

fn build_cat_pattern_2d(state: &CssRunnerState) -> Node {
    let panels = &state.project.template.panels;
    let selected = panels.get(state.selected_piece_index);
    let name = selected.map_or("None", |p| p.id.as_str());
    let fabric = selected.map_or("Default", |p| p.fabric.as_str());
    let grain = selected.map_or(0.0, |p| p.grain_degrees);
    let count = panels.len();

    let mut piece_rows = Vec::new();
    for (idx, p) in panels.iter().enumerate() {
        let is_sel = idx == state.selected_piece_index;
        let item_class = if is_sel {
            "tree-item active"
        } else {
            "tree-item"
        };
        let label = format!("{}: {}", p.id, p.name);
        piece_rows.push(ui! {
            <div class={item_class}>
                <span>{label}</span>
                <span>{format!("{:.0}°", p.grain_degrees)}</span>
            </div>
        });
    }

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Pattern Pieces"</h3>
            <div class="prop-row">
                <span class="prop-label">"Total Pieces"</span>
                <span class="prop-value">{format!("{count}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Selected ID"</span>
                <span class="prop-value">{name}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Fabric Preset"</span>
                <span class="prop-value">{fabric}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Grain Angle"</span>
                <span class="prop-value">{format!("{grain:.1}°")}</span>
            </div>
            <div class="browser-tree" style="margin-top:6px">
                {piece_rows}
            </div>
        </div>
    }
}

fn build_cat_sewing(state: &CssRunnerState) -> Node {
    let seams = &state.project.template.seams;
    let seams_count = seams.len();

    let mut seam_rows = Vec::new();
    for (idx, s) in seams.iter().enumerate() {
        let label = format!("{}: {} ↔ {}", s.id, s.a.panel, s.b.panel);
        let rev_badge = if s.reverse_b { " (rev)" } else { "" };
        seam_rows.push(ui! {
            <div class="tree-item">
                <span>{label}</span>
                <span style="color:var(--accent);font-size:10px">{format!("#{idx}{rev_badge}")}</span>
            </div>
        });
    }

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Sewing Seams"</h3>
            <div class="prop-row">
                <span class="prop-label">"Total Seams"</span>
                <span class="prop-value">{format!("{seams_count}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Active Tool"</span>
                <span class="prop-value">{state.active_2d_tool.clone()}</span>
            </div>
            <div class="browser-tree" style="margin-top:6px">
                {seam_rows}
            </div>
        </div>
    }
}

fn build_cat_form_3d(state: &CssRunnerState) -> Node {
    let verts = state.scene.garment.mesh.uv.len();
    let tris = state.scene.garment.mesh.triangles.len();
    let pins_count = state
        .project
        .template
        .panels
        .iter()
        .map(|p| p.pins.len() + p.pin_points.len())
        .sum::<usize>();

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"3D Form & Positioning"</h3>
            <div class="prop-row">
                <span class="prop-label">"Garment Vertices"</span>
                <span class="prop-value">{format!("{verts}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Garment Triangles"</span>
                <span class="prop-value">{format!("{tris}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Pinned Points"</span>
                <span class="prop-value">{format!("{pins_count}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Active 3D Tool"</span>
                <span class="prop-value">{state.active_3d_tool.clone()}</span>
            </div>
        </div>
    }
}

fn build_cat_material(state: &CssRunnerState) -> Node {
    let mat_count = state.scene.garment.materials.len();
    let fabrics_count = state.project.template.fabrics.len();

    let mut fabric_rows = Vec::new();
    for f in &state.project.template.fabrics {
        let dens = format!("{:.0} g/m²", f.properties.density * 1000.0);
        fabric_rows.push(ui! {
            <div class="tree-item">
                <span>{f.name.clone()}</span>
                <span style="color:var(--text-muted);font-size:10px">{dens}</span>
            </div>
        });
    }

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Fabric & Appearance"</h3>
            <div class="prop-row">
                <span class="prop-label">"Defined Fabrics"</span>
                <span class="prop-value">{format!("{fabrics_count}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Materials Bound"</span>
                <span class="prop-value">{format!("{mat_count}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Albedo Maps"</span>
                <span class="prop-value">"Portable Linear-Light"</span>
            </div>
            <div class="browser-tree" style="margin-top:6px">
                {fabric_rows}
            </div>
        </div>
    }
}

fn build_cat_environment(state: &CssRunnerState) -> Node {
    let avatar_tris = state.scene.collider.surface.triangles.len();
    let clearance_mm = format!("{:.1} mm", state.project.contact_clearance * 1000.0);

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Avatar & Environment"</h3>
            <div class="prop-row">
                <span class="prop-label">"Avatar Triangles"</span>
                <span class="prop-value">{format!("{avatar_tris}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Contact Clearance"</span>
                <span class="prop-value">{clearance_mm}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Profiles Store"</span>
                <span class="prop-value">"workspace-profiles.json"</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Status"</span>
                <span class="prop-value">"Synced"</span>
            </div>
        </div>
    }
}

fn build_runner_property_accordions(state: &CssRunnerState) -> Node {
    let panels = &state.project.template.panels;
    let selected_panel = panels.get(state.selected_piece_index);
    let selected_name = selected_panel.map_or("None", |p| p.name.as_str());
    let selected_id = selected_panel.map_or("none", |p| p.id.as_str());

    let (width_mm, height_mm) = if let Some(p) = selected_panel {
        if let Some(ref poly) = p.polygon {
            let min_x = poly
                .points
                .iter()
                .map(|pt| pt.x)
                .fold(f64::INFINITY, f64::min);
            let max_x = poly
                .points
                .iter()
                .map(|pt| pt.x)
                .fold(f64::NEG_INFINITY, f64::max);
            let min_y = poly
                .points
                .iter()
                .map(|pt| pt.y)
                .fold(f64::INFINITY, f64::min);
            let max_y = poly
                .points
                .iter()
                .map(|pt| pt.y)
                .fold(f64::NEG_INFINITY, f64::max);
            ((max_x - min_x) * 1000.0, (max_y - min_y) * 1000.0)
        } else {
            (p.pattern.width * 1000.0, p.pattern.height * 1000.0)
        }
    } else {
        (0.0, 0.0)
    };

    let area_dm2 = (width_mm * height_mm) / 10000.0;
    let perimeter_mm = (width_mm + height_mm) * 2.0;

    let last_pen = state
        .last_metrics
        .as_ref()
        .map_or("0.00 mm".to_string(), |m| {
            format!("{:.2} mm", m.minimum_distance_m * 1000.0)
        });
    let last_res = state.last_metrics.as_ref().map_or("N/A".to_string(), |m| {
        m.barrier_al.as_ref().map_or_else(
            || {
                m.residual_rms_n
                    .map_or("N/A".into(), |r| format!("{r:.1e}"))
            },
            |al| format!("{:.1e} N max", al.original_force_max_n),
        )
    });

    let pins_count = selected_panel.map_or(0, |p| p.pins.len() + p.pin_points.len());

    let open_idx = state.open_accordion_idx;

    // Accordion 0: Information
    let acc_info = ui! {
        <div class="accordion">
            <button class="accordion-header" onclick={on_runner_accordion_toggle}>
                <span>"1. Piece Information"</span>
                {if open_idx == 0 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            <div class="accordion-content">
                <div class="prop-row">
                    <span class="prop-label">"Name"</span>
                    <span class="prop-value">{selected_name}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"ID"</span>
                    <span class="prop-value">{selected_id}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Width"</span>
                    <span class="prop-value">{format!("{width_mm:.1} mm")}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Height"</span>
                    <span class="prop-value">{format!("{height_mm:.1} mm")}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Area"</span>
                    <span class="prop-value">{format!("{area_dm2:.2} dm²")}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Perimeter"</span>
                    <span class="prop-value">{format!("{perimeter_mm:.1} mm")}</span>
                </div>
            </div>
        </div>
    };

    // Accordion 1: Selected Line & Seam Allowance
    let acc_line = ui! {
        <div class="accordion">
            <button class="accordion-header" onclick={on_runner_accordion_toggle}>
                <span>"2. Boundary & Allowance"</span>
                {if open_idx == 1 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            <div class="accordion-content">
                <div class="prop-row">
                    <span class="prop-label">"Curvature Type"</span>
                    <span class="prop-value">"Cubic Bézier"</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Seam Allowance"</span>
                    <span class="prop-value">"10.0 mm"</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Cutting Line Offset"</span>
                    <span class="prop-value">"+10.0 mm"</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Notches"</span>
                    <span class="prop-value">"Stored (V-Cut)"</span>
                </div>
            </div>
        </div>
    };

    // Accordion 2: 3D Arrangement
    let acc_arrange = ui! {
        <div class="accordion">
            <button class="accordion-header" onclick={on_runner_accordion_toggle}>
                <span>"3. 3D Arrangement"</span>
                {if open_idx == 2 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            <div class="accordion-content">
                <div class="prop-row">
                    <span class="prop-label">"Translation X"</span>
                    <span class="prop-value">{format!("{:.1} cm", selected_panel.map_or(0.0, |p| p.placement.translation.x * 100.0))}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Translation Y"</span>
                    <span class="prop-value">{format!("{:.1} cm", selected_panel.map_or(0.0, |p| p.placement.translation.y * 100.0))}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Translation Z"</span>
                    <span class="prop-value">{format!("{:.1} cm", selected_panel.map_or(0.0, |p| p.placement.translation.z * 100.0))}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Rotation (X/Y/Z)"</span>
                    <span class="prop-value">{format!("{:.0}°/{:.0}°/{:.0}°",
                        selected_panel.map_or(0.0, |p| p.placement.rotation_degrees.x),
                        selected_panel.map_or(0.0, |p| p.placement.rotation_degrees.y),
                        selected_panel.map_or(0.0, |p| p.placement.rotation_degrees.z))}</span>
                </div>
            </div>
        </div>
    };

    // Accordion 3: Pins
    let acc_pins = ui! {
        <div class="accordion">
            <button class="accordion-header" onclick={on_runner_accordion_toggle}>
                <span>"4. Pins & Restraints"</span>
                {if open_idx == 3 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            <div class="accordion-content">
                <div class="prop-row">
                    <span class="prop-label">"Pinned Points"</span>
                    <span class="prop-value">{format!("{pins_count}")}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Active Pin Mode"</span>
                    <span class="prop-value">{if state.pin_mode_contour { "Contour" } else { "Interior" }}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Pin Stiffness"</span>
                    <span class="prop-value">"1e5 N/m"</span>
                </div>
            </div>
        </div>
    };

    // Accordion 4: Fabric & Mechanical Properties
    let acc_fabric = ui! {
        <div class="accordion">
            <button class="accordion-header" onclick={on_runner_accordion_toggle}>
                <span>"5. Fabric Mechanics"</span>
                {if open_idx == 4 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            <div class="accordion-content">
                <div class="prop-row">
                    <span class="prop-label">"Preset"</span>
                    <span class="prop-value">{selected_panel.map_or("Cotton", |p| p.fabric.as_str())}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Areal Density"</span>
                    <span class="prop-value">"180 g/m²"</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Warp Stretch"</span>
                    <span class="prop-value">"150 N/m"</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Weft Stretch"</span>
                    <span class="prop-value">"120 N/m"</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Bending Stiffness"</span>
                    <span class="prop-value">"0.002 N·m"</span>
                </div>
            </div>
        </div>
    };

    // Accordion 5: Simulation Diagnostics
    let acc_sim = ui! {
        <div class="accordion">
            <button class="accordion-header" onclick={on_runner_accordion_toggle}>
                <span>"6. Simulation Diagnostics"</span>
                {if open_idx == 5 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            <div class="accordion-content">
                <div class="prop-row">
                    <span class="prop-label">"Solver Step Time"</span>
                    <span class="prop-value">{format!("{:.2} ms", state.step_ms)}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"RMS Residual"</span>
                    <span class="prop-value">{last_res}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Max Penetration"</span>
                    <span class="prop-value">{last_pen}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Active Backend"</span>
                    <span class="prop-value">{if state.gpu_sim { state.contact_method.label() } else { "CPU (OGC)" }}</span>
                </div>
            </div>
        </div>
    };

    ui! {
        <div class="accordions-container">
            {acc_info}
            {acc_line}
            {acc_arrange}
            {acc_pins}
            {acc_fabric}
            {acc_sim}
        </div>
    }
}

fn build_runner_status_bar(state: &CssRunnerState) -> Node {
    let step_info = format!(
        "Step: {} | Worker: {:.1} ms | Status: {} | Tool: {} / {}",
        state.step_count,
        state.step_ms,
        state.status_message,
        state.active_3d_tool,
        state.active_2d_tool
    );
    let mesh_info = format!(
        "Cloth: {} verts, {} tris | Avatar: {} tris",
        state.scene.state.x.len(),
        state.scene.garment.mesh.triangles.len(),
        state.scene.collider.surface.triangles.len()
    );

    ui! {
        <footer class="status-bar">
            <div class="status-left">
                <span class="status-indicator">"●"</span>
                <span class="status-message">{step_info}</span>
            </div>
            <div class="status-right">
                <span class="status-mesh-info">{mesh_info}</span>
            </div>
        </footer>
    }
}

fn build_runner_command_palette(state: &CssRunnerState) -> Node {
    if !state.command_palette_open {
        return ui! { <div id="command-palette-container"></div> };
    }

    ui! {
        <div id="command-palette-container" class="command-palette-backdrop">
            <div class="command-palette-modal">
                <div class="palette-header">
                    <span class="palette-title">"Studio V2 Command Palette"</span>
                    <button class="palette-close-btn" onclick={on_runner_palette_toggle}>"✕"</button>
                </div>
                <div class="palette-input">
                    "Type a command or shortcut..."
                </div>
                <div class="palette-items">
                    <button class="palette-item" onclick={on_runner_play_pause}>
                        <span>"▶ Toggle Simulation"</span>
                        <span class="command-shortcut">"Space"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_step}>
                        <span>"⏭ Step Simulation"</span>
                        <span class="command-shortcut">"S"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_reset}>
                        <span>"↺ Reset Simulation"</span>
                        <span class="command-shortcut">"R"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_save}>
                        <span>"💾 Save Fitting Project"</span>
                        <span class="command-shortcut">"Ctrl+S"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_export}>
                        <span>"📦 Export Artifacts (OBJ & True-Scale SVG)"</span>
                        <span class="command-shortcut">"Ctrl+E"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_clean_view}>
                        <span>"👁 Toggle Clean View"</span>
                        <span class="command-shortcut">"F11"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_theme_toggle}>
                        <span>"🎨 Toggle Dark / Light Theme"</span>
                        <span class="command-shortcut">"T"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_view_split}>
                        <span>"◫ Split 2D / 3D Layout"</span>
                        <span class="command-shortcut">"1"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_view_3d}>
                        <span>"◻ 3D Only Viewport"</span>
                        <span class="command-shortcut">"2"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_view_2d}>
                        <span>"◰ 2D Only Pattern Board"</span>
                        <span class="command-shortcut">"3"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_cam_reset}>
                        <span>"⌖ Reset 3D Camera"</span>
                        <span class="command-shortcut">"Home"</span>
                    </button>
                    <button class="palette-item" onclick={on_runner_drawer_toggle}>
                        <span>"☰ Toggle Left Asset Drawer"</span>
                        <span class="command-shortcut">"Ctrl+B"</span>
                    </button>
                </div>
            </div>
        </div>
    }
}

// ============================================================================
// 2D Pattern Board SVG
// ============================================================================

fn render_runner_2d_svg(state: &CssRunnerState) -> (Vec<Node>, String) {
    let mut nodes = Vec::new();
    let panels = &state.project.template.panels;

    struct PanelGeo {
        id: String,
        points: Vec<(f64, f64)>,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        center: (f64, f64),
        grain_deg: f64,
        is_selected: bool,
    }

    let mut panel_geos = Vec::new();
    let mut auto_x = 0.0_f64;

    for (idx, panel) in panels.iter().enumerate() {
        let is_selected = idx == state.selected_piece_index;
        let mut raw_points = Vec::new();

        if let Some(ref poly) = panel.polygon {
            for pt in &poly.points {
                raw_points.push((pt.x, pt.y));
            }
        }
        if raw_points.is_empty() {
            let w = panel.pattern.width.max(0.1);
            let h = panel.pattern.height.max(0.1);
            raw_points.push((0.0, 0.0));
            raw_points.push((w, 0.0));
            raw_points.push((w, h));
            raw_points.push((0.0, h));
        }

        let p_min_x = raw_points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let p_max_x = raw_points
            .iter()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let _p_min_y = raw_points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let _p_max_y = raw_points
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let width = (p_max_x - p_min_x).max(0.05);

        let (offset_x, offset_y) = if let Some(l2d) = panel.layout_2d {
            (l2d.x, l2d.y)
        } else {
            let ox = auto_x - p_min_x;
            auto_x += width + 0.12;
            (ox, 0.0)
        };

        let shifted: Vec<(f64, f64)> = raw_points
            .iter()
            .map(|&(x, y)| (x + offset_x, y + offset_y))
            .collect();

        let s_min_x = shifted.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let s_max_x = shifted
            .iter()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let s_min_y = shifted.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let s_max_y = shifted
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let center = ((s_min_x + s_max_x) * 0.5, (s_min_y + s_max_y) * 0.5);

        panel_geos.push(PanelGeo {
            id: panel.id.clone(),
            points: shifted,
            min_x: s_min_x,
            min_y: s_min_y,
            max_x: s_max_x,
            max_y: s_max_y,
            center,
            grain_deg: panel.grain_degrees,
            is_selected,
        });
    }

    let global_min_x = panel_geos.iter().map(|p| p.min_x).fold(0.0_f64, f64::min);
    let global_max_x = panel_geos.iter().map(|p| p.max_x).fold(0.6_f64, f64::max);
    let global_min_y = panel_geos.iter().map(|p| p.min_y).fold(0.0_f64, f64::min);
    let global_max_y = panel_geos.iter().map(|p| p.max_y).fold(0.6_f64, f64::max);

    let span_x = (global_max_x - global_min_x).max(0.2);
    let span_y = (global_max_y - global_min_y).max(0.2);
    let margin = (span_x.max(span_y) * 0.12).max(0.08);

    let vb_min_x = global_min_x - margin;
    let vb_min_y = global_min_y - margin;
    let vb_w = span_x + 2.0 * margin;
    let vb_h = span_y + 2.0 * margin;
    let view_box = format!("{vb_min_x:.3} {vb_min_y:.3} {vb_w:.3} {vb_h:.3}");

    // Background rect
    nodes.push(ui! {
        <rect x={format!("{vb_min_x:.3}")} y={format!("{vb_min_y:.3}")} width={format!("{vb_w:.3}")} height={format!("{vb_h:.3}")} fill="#0d0f15" />
    });

    // Drafting grid (every 0.1m / 100mm)
    let grid_step = 0.10_f64;
    let start_gx = (vb_min_x / grid_step).floor() * grid_step;
    let mut gx = start_gx;
    while gx <= vb_min_x + vb_w {
        let is_major = (gx.abs() % 0.5).abs() < 0.01;
        let stroke = if is_major { "#252b3b" } else { "#181d28" };
        let sw = if is_major { vb_w * 0.002 } else { vb_w * 0.001 };
        let pts = format!("{gx:.3},{:.3} {gx:.3},{:.3}", vb_min_y, vb_min_y + vb_h);
        nodes.push(ui! {
            <polyline points={pts} stroke={stroke} stroke-width={format!("{sw:.4}")} />
        });
        gx += grid_step;
    }
    let start_gy = (vb_min_y / grid_step).floor() * grid_step;
    let mut gy = start_gy;
    while gy <= vb_min_y + vb_h {
        let is_major = (gy.abs() % 0.5).abs() < 0.01;
        let stroke = if is_major { "#252b3b" } else { "#181d28" };
        let sw = if is_major { vb_w * 0.002 } else { vb_w * 0.001 };
        let pts = format!("{:.3},{gy:.3} {:.3},{gy:.3}", vb_min_x, vb_min_x + vb_w);
        nodes.push(ui! {
            <polyline points={pts} stroke={stroke} stroke-width={format!("{sw:.4}")} />
        });
        gy += grid_step;
    }

    // Panels
    for p in &panel_geos {
        let stroke_color = if p.is_selected { "#f59e0b" } else { "#3b82f6" };
        let fill_color = if p.is_selected {
            "rgba(245, 158, 11, 0.14)"
        } else {
            "rgba(59, 130, 246, 0.08)"
        };
        let stroke_w = if p.is_selected {
            vb_w * 0.005
        } else {
            vb_w * 0.0025
        };

        let mut pts_str = String::new();
        for (x, y) in &p.points {
            if !pts_str.is_empty() {
                pts_str.push(' ');
            }
            pts_str.push_str(&format!("{x:.3},{y:.3}"));
        }

        nodes.push(ui! {
            <polygon points={pts_str} fill={fill_color} stroke={stroke_color} stroke-width={format!("{stroke_w:.4}")} />
        });

        // Grainline
        let rad = p.grain_deg.to_radians();
        let grain_len = ((p.max_x - p.min_x).min(p.max_y - p.min_y) * 0.35).max(0.05);
        let (cx, cy) = p.center;
        let gx0 = cx - grain_len * rad.sin();
        let gy0 = cy - grain_len * rad.cos();
        let gx1 = cx + grain_len * rad.sin();
        let gy1 = cy + grain_len * rad.cos();
        let g_pts = format!("{gx0:.3},{gy0:.3} {gx1:.3},{gy1:.3}");
        nodes.push(ui! {
            <polyline points={g_pts} stroke="#60a5fa" stroke-width={format!("{:.4}", vb_w * 0.002)} />
        });

        // Label
        let font_size = format!("{:.3}", (vb_w * 0.025).max(0.015));
        nodes.push(ui! {
            <text x={format!("{cx:.3}")} y={format!("{:.3}", cy - grain_len * 0.4)} fill="#cbd5e1" font-size={font_size} font-family="sans-serif">
                {p.id.clone()}
            </text>
        });
    }

    // Seam links
    for seam in &state.project.template.seams {
        let p_a = panel_geos.iter().find(|p| p.id == seam.a.panel);
        let p_b = panel_geos.iter().find(|p| p.id == seam.b.panel);
        if let (Some(a), Some(b)) = (p_a, p_b) {
            let pts = format!(
                "{:.3},{:.3} {:.3},{:.3}",
                a.center.0, a.center.1, b.center.0, b.center.1
            );
            nodes.push(ui! {
                <polyline points={pts} stroke="#10b981" stroke-dasharray={format!("{:.3},{:.3}", vb_w * 0.01, vb_w * 0.008)} stroke-width={format!("{:.4}", vb_w * 0.002)} />
            });
        }
    }

    (nodes, view_box)
}

// ============================================================================
// App Constructor and Entry Point
// ============================================================================

#[allow(clippy::type_complexity)]
pub fn create_runner_app<'a>(
    mut state: CssRunnerState,
    stylesheet: &'a Stylesheet,
) -> App<
    'a,
    CssRunnerState,
    fn(&mut CssRunnerState, FrameInfo) -> Invalidation,
    fn(&CssRunnerState) -> Node,
    Invalidation,
> {
    let preview_path = state.files.output.join("runner_preview.usda");
    if let Some(parent) = preview_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let usda =
        marvelous_cloth::usd_scene::garment_stage_usda(&state.scene.garment, &state.scene.state)
            .unwrap_or_default();
    let usda =
        marvelous_cloth::usd_scene::append_viewport_surface(usda, &state.scene.collider.surface);
    let _ = std::fs::write(&preview_path, usda);

    let usd_scene = cssimpler::renderer::gpu::usd::UsdScene::open(&preview_path)
        .expect("load generated USD viewport scene");
    state.usd_scene = Some(usd_scene.clone());

    let smoke = state.smoke;
    let smoke_done = state.smoke_done.clone();
    let smoke_output = state.files.output.join("ui.png");
    let app = App::new(
        state,
        stylesheet,
        update_runner as fn(&mut CssRunnerState, FrameInfo) -> Invalidation,
        build_runner_ui as fn(&CssRunnerState) -> Node,
    )
    .with_event_handler(handle_runner_event)
    .with_render_mode(RenderMode::OnInvalidation)
    .with_continuous_updates(true)
    .with_exit_when(|state| state.smoke_done.get() || state.should_exit);

    if smoke {
        app.with_gpu_viewport(
            "runner-viewport-3d-scene",
            Default::default(),
            move |init| {
                let redraw = init.redraw.clone();
                let renderer = cssimpler::renderer::gpu::usd::UsdRenderer::new(
                    init,
                    usd_scene,
                    cssimpler::renderer::gpu::usd::UsdCameraMode::Orbit,
                )?;
                Ok(Box::new(super::usd_smoke::SmokeViewport {
                    renderer,
                    output: smoke_output,
                    done: smoke_done,
                    redraw,
                    frames: 0,
                }))
            },
        )
    } else {
        app.with_usd_viewport(
            "runner-viewport-3d-scene",
            usd_scene,
            cssimpler::renderer::gpu::usd::UsdCameraMode::Orbit,
        )
    }
}

fn init_gpu_device() -> Result<(wgpu::Device, wgpu::Queue), String> {
    marvelous_cloth::gpu_avbd::request_device()
}

pub fn run(
    project: FittingProject,
    scene: FittingScene,
    files: DocumentFiles,
    options: super::RunOptions,
) -> Result<(), String> {
    let super::RunOptions {
        barrier_al,
        smoke,
        drag_test,
        perf_test,
        authoring_test,
        gpu_sim,
    } = options;
    let stylesheet =
        Box::leak(Box::new(load_studio_stylesheet().map_err(|e| {
            format!("Failed to load Studio V2 stylesheet: {e}")
        })?));

    let (tx, latest) = if gpu_sim {
        let (device, queue) = init_gpu_device()?;
        worker::start(device, queue).map_err(|e| format!("Failed to start GPU worker: {e}"))?
    } else {
        worker::start_switchable().map_err(|e| format!("Failed to start CPU worker: {e}"))?
    };

    let mut state = CssRunnerState::new(project, scene, files, tx, latest);
    state.smoke = smoke;
    state.drag_test = drag_test;
    state.perf_test = perf_test;
    state.authoring_test = authoring_test;
    state.gpu_sim = gpu_sim;
    state.contact_method = if barrier_al {
        ContactMethod::BarrierAl
    } else {
        ContactMethod::Ipc
    };
    if perf_test {
        state.perf = Some(super::perf::PerfRun::new());
        state.playing = true;
        state.send_control_state(false);
    }
    let smoke_done = state.smoke_done.clone();
    let app = create_runner_app(state, stylesheet);
    let config = WindowConfig::new("ClothCraft | Simulation Studio V2", 1600, 980);
    app.run(config)
        .map_err(|e| format!("CSSimpler application error: {e}"))?;
    if smoke && !smoke_done.get() {
        return Err("Native smoke closed before USD capture completed".into());
    }
    Ok(())
}

// ============================================================================
// Unit Tests (Headless Frame & RGBA8 verification)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use marvelous_cloth::garment::presets::GarmentKind;
    use std::time::Duration;

    #[test]
    fn test_css_runner_headless_frame_and_rendering() {
        let stylesheet = load_studio_stylesheet().expect("load studio stylesheet");
        let dir = tempfile::tempdir().expect("tempdir");
        let avatar = dir.path().join("avatar.obj");
        std::fs::write(&avatar, "v 0 0 -10\nv 1 0 -10\nv 0 1 -10\nf 1 2 3\n")
            .expect("write avatar");
        let project = FittingProject::basic(GarmentKind::Tshirt, 1, avatar);
        let scene = project.compile().expect("compile scene");
        let files = DocumentFiles {
            output: dir.path().join("test_runner_out"),
            source: None,
            modified: false,
        };

        let (tx, latest) = worker::start_cpu().expect("start cpu worker");
        let state = CssRunnerState::new(project, scene, files, tx, latest);
        let mut app = create_runner_app(state, &stylesheet);
        app.set_viewport(cssimpler::renderer::ViewportSize::new(1600, 980));

        // Headless frame advance
        let scene = app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 0,
        });
        assert!(!scene.is_empty(), "runner scene should not be empty");

        let collider_points = app
            .state()
            .usd_scene
            .as_ref()
            .unwrap()
            .stage()
            .attribute("/Collider.points")
            .unwrap()
            .get::<Vec<cssimpler::renderer::gpu::openusd::gf::Vec3f>>()
            .unwrap()
            .unwrap();
        assert!(
            !collider_points.is_empty(),
            "collision geometry must be visible in USD"
        );
        // Verify layout containers
        assert!(app.find_element_bounds_by_id("#viewport-3d").is_some());
        let usd_bounds = app
            .find_element_bounds_by_id("#runner-viewport-3d-scene")
            .expect("USD viewport exists");
        assert!(
            usd_bounds.width > 100.0 && usd_bounds.height > 100.0,
            "USD viewport must have drawable area: {usd_bounds:?}"
        );
        assert!(app.find_element_bounds_by_id("#viewport-2d").is_some());
        assert!(app.find_element_bounds_by_id("#right-dock").is_some());

        // The menu is rendered in the live app; changing backend transfers the
        // displayed checkpoint without stepping or initializing a GPU.
        let checkpoint = app.state().scene.state.clone();
        let generation = app.state().generation;
        let button = app
            .find_element_bounds_by_id("#sim-backend-indicator")
            .expect("solver button");
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 10,
        });
        assert!(app.state().solver_menu_open);
        let menu = app
            .find_element_bounds_by_id("#solver-dropdown")
            .expect("open solver menu");
        assert!(menu.width > 100.0 && menu.height > 20.0);
        assert!(app.find_element_bounds_by_id("#solver-wgsl-ipc").is_some());
        let button = app
            .find_element_bounds_by_id("#solver-wgsl-ipc")
            .expect("solver button");
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 11,
        });
        assert!(app.state().gpu_sim);
        assert!(!app.state().playing && !app.state().solver_menu_open);
        assert_eq!(app.state().generation, generation + 1);
        assert_eq!(app.state().scene.state.x, checkpoint.x);
        assert_eq!(app.state().scene.state.v, checkpoint.v);
        assert_eq!(app.state().scene.state.time, checkpoint.time);
        let button = app
            .find_element_bounds_by_id("#sim-backend-indicator")
            .unwrap();
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 12,
        });
        let button = app.find_element_bounds_by_id("#solver-barrier-al").unwrap();
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 13,
        });
        assert_eq!(app.state().contact_method, ContactMethod::BarrierAl);
        assert_eq!(app.state().generation, generation + 2);
        assert_eq!(app.state().scene.state.x, checkpoint.x);
        let button = app
            .find_element_bounds_by_id("#sim-backend-indicator")
            .unwrap();
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 12,
        });
        let button = app.find_element_bounds_by_id("#solver-cpu").unwrap();
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 12,
        });
        assert!(!app.state().gpu_sim);
        assert_eq!(app.state().generation, generation + 3);
        assert_eq!(app.state().scene.state.x, checkpoint.x);

        // Headless RGBA8 rendering
        let width = 640;
        let height = 480;
        let mut rgba = vec![0_u8; width * height * 4];
        app.render_rgba8(
            FrameInfo {
                delta: Duration::from_millis(16),
                frame_index: 1,
            },
            &mut rgba,
            width,
            height,
            1.0,
            cssimpler::core::Color::rgba(0, 0, 0, 0),
        );
        assert!(
            rgba.iter().any(|&b| b > 0),
            "rendered buffer should have painted pixels"
        );

        // Deliver an accepted worker snapshot without running GPU work.
        let read_points = |state: &CssRunnerState| {
            let path =
                marvelous_cloth::usd_scene::viewport_panel_path(&state.scene.garment.panels[0].id);
            state
                .usd_scene
                .as_ref()
                .unwrap()
                .stage()
                .attribute(format!("{path}.points"))
                .unwrap()
                .get::<Vec<cssimpler::renderer::gpu::openusd::gf::Vec3f>>()
                .unwrap()
                .unwrap()
        };
        let initial_points = read_points(app.state());
        let mut moved = app.state().scene.state.clone();
        moved.x[0].y += 0.01;
        *app.state().worker_latest.lock().unwrap() = Some(worker::Done {
            generation: app.state().generation,
            had_grab: false,
            result: Ok((moved, VbdMetrics::default())),
        });
        update_runner(
            app.state_mut(),
            FrameInfo {
                delta: Duration::from_millis(16),
                frame_index: 2,
            },
        );
        assert_ne!(read_points(app.state()), initial_points);
        app.set_viewport(cssimpler::renderer::ViewportSize::new(1600, 980));
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 3,
        });
        let bounds = app
            .find_element_bounds_by_id("#runner-viewport-3d-scene")
            .unwrap();
        assert!(
            bounds.width > 100.0
                && bounds.height > 100.0
                && bounds.x >= 0.0
                && bounds.x + bounds.width <= 1600.0
                && bounds.y + bounds.height <= 980.0,
            "live USD host must remain inside the window: {bounds:?}"
        );

        app.state_mut().reset_simulation();
        assert_eq!(read_points(app.state()), initial_points);

        // Native USD hosts are hidden if later text paint overlaps their area.
        // Check expanded diagnostics as well as Ready, across window sizes.
        for (width, height) in [(1600, 980), (1280, 800), (1024, 768), (800, 600)] {
            app.set_viewport(cssimpler::renderer::ViewportSize::new(width, height));
            for message in ["Ready", "Step 124: min dist 0.124 mm, res 0.0e0, 65.2 ms"] {
                app.state_mut().status_message = message.into();
                let roots = app.frame(FrameInfo {
                    delta: Duration::from_millis(16),
                    frame_index: 4,
                });
                let viewport = app
                    .find_element_bounds_by_id("#runner-viewport-3d-scene")
                    .unwrap();
                let paint = cssimpler::core::ExtractedScene::from_render_roots(&roots);
                for item in paint
                    .items
                    .iter()
                    .filter(|item| item.kind == cssimpler::core::ExtractedPaintKind::TextRun)
                {
                    let text = item.layout;
                    let overlaps = text.x < viewport.x + viewport.width
                        && viewport.x < text.x + text.width
                        && text.y < viewport.y + viewport.height
                        && viewport.y < text.y + text.height;
                    assert!(
                        !overlaps,
                        "text must not hide USD at {width}x{height}: {:?} {:?} over {viewport:?}",
                        item.text, text
                    );
                }
            }
        }

        // Test Play / Step
        app.state_mut().playing = true;
        app.state_mut().trigger_worker_step(true);
        assert!(app.state().worker_pending);

        // Test Reset
        app.state_mut().reset_simulation();
        assert!(!app.state().playing);
        assert_eq!(app.state().step_count, 0);

        // Test Category Tab Switching
        assert_eq!(app.state().active_category, ObjectCategory::Pattern2D);
        app.state_mut().active_category = ObjectCategory::Sewing;
        assert_eq!(app.state().active_category, ObjectCategory::Sewing);

        // Test Tool Switching
        app.state_mut().active_2d_tool = "Corner".to_string();
        assert_eq!(app.state().active_2d_tool, "Corner");
    }
}

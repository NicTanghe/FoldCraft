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
use std::sync::{Arc, Mutex, mpsc};

use cssimpler::app::{App, Invalidation, RenderMode};
use cssimpler::core::Node;
use cssimpler::renderer::{EngineEvent, FrameInfo, WindowConfig};
use cssimpler::style::Stylesheet;
use cssimpler::ui;

use glam::DVec2;
use marvelous_cloth::authoring::{
    self, Command, CornerRule, History,
    corner_tool::{
        CIRCLE_TENSION, FilletPreview, max_fillet_radius, preview_fillet, validate_fillet_preview,
    },
};
use marvelous_cloth::css_ui::icons::*;
use marvelous_cloth::css_ui::{Theme, ViewLayoutMode, load_studio_stylesheet};
use marvelous_cloth::fitting::{FittingProject, FittingScene};
use marvelous_cloth::garment::{
    GarmentTemplate, curves, export, polygon::PolygonPattern, template::Panel,
};
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
pub const ACT_RUN_ACC_0: u64 = 1 << 41;
pub const ACT_RUN_ACC_1: u64 = 1 << 42;
pub const ACT_RUN_ACC_2: u64 = 1 << 43;
pub const ACT_RUN_ACC_3: u64 = 1 << 44;
pub const ACT_RUN_ACC_4: u64 = 1 << 45;
pub const ACT_RUN_ACC_5: u64 = 1 << 46;
pub const ACT_RUN_ACCORDION_TOGGLE: u64 = 1 << 47;
pub const ACT_RUN_PIECE_0: u64 = 1 << 48;
pub const ACT_RUN_PIECE_1: u64 = 1 << 49;
pub const ACT_RUN_PIECE_2: u64 = 1 << 50;
pub const ACT_RUN_PIECE_3: u64 = 1 << 51;

pub const ACT_RUN_SOLVER_MENU: u64 = 1 << 52;
pub const ACT_RUN_SOLVER_GPU: u64 = 1 << 53;
pub const ACT_RUN_SOLVER_CPU: u64 = 1 << 54;

pub const ACT_RUN_SOLVER_AL: u64 = 1 << 55;
pub const ACT_RUN_2D_ZOOM_IN: u64 = 1 << 56;
pub const ACT_RUN_2D_ZOOM_OUT: u64 = 1 << 57;
pub const ACT_RUN_2D_FIT: u64 = 1 << 58;
pub const ACT_RUN_2D_NEXT_PIECE: u64 = 1 << 59;

// ============================================================================
// Object Browser Category & 2D Interactive Types
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect2d {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect2d {
    pub fn contains(self, px: f32, py: f32) -> bool {
        self.w > 0.0
            && self.h > 0.0
            && px >= self.x
            && px <= self.x + self.w
            && py >= self.y
            && py <= self.y + self.h
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Hover2dTarget {
    Panel(usize),
    Edge {
        panel_idx: usize,
        edge_idx: usize,
    },
    Point {
        panel_idx: usize,
        point_idx: usize,
    },
    BezierHandle {
        panel_idx: usize,
        edge_idx: usize,
        handle_index: usize,
    },
    CornerPreviewHandle {
        panel_idx: usize,
        edge_name: String,
    },
}

#[derive(Clone, Debug)]
pub enum Drag2d {
    Pan {
        start_pan: (f64, f64),
        start_pointer: (f32, f32),
    },
    Point {
        panel_id: String,
        point_idx: usize,
        start_pointer: (f32, f32),
        moved: bool,
    },
    BezierHandle {
        panel_id: String,
        edge_name: String,
        handle_index: usize,
        start_pointer: (f32, f32),
        moved: bool,
        last_local: DVec2,
    },
    CornerRadius {
        panel_id: String,
        edge_name: String,
        corner_board_m: DVec2,
        bisector: DVec2,
        denom: f64,
        max_radius_m: f64,
        start_pointer: (f32, f32),
        moved: bool,
    },
}

#[derive(Clone, Debug)]
pub enum RunnerUiOp {
    SetCornerMode(bool),
    SetCornerRule(CornerRule),
    AdjustCornerRadiusMm(f64),
    AdjustCornerTension(f64),
    ApplyCornerRoundSelected,
    ToggleSelectedCurve(bool),
    Undo2d,
    Redo2d,
}

// ============================================================================
// State Struct
// ============================================================================

pub struct CssRunnerState {
    actions: Arc<AtomicU64>,
    pending_ui_ops: Arc<Mutex<Vec<RunnerUiOp>>>,
    pub smoke: bool,
    pub smoke_done: std::rc::Rc<std::cell::Cell<bool>>,
    pub usd_scene: Option<cssimpler::renderer::gpu::usd::UsdScene>,
    pub project: FittingProject,
    pub preview_template: Option<GarmentTemplate>,
    pub history: History,
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
    pub selected_2d_point: Option<(String, usize)>,
    pub selected_2d_edge: Option<(String, String)>,
    pub hovered_2d: Option<Hover2dTarget>,
    pub drag_2d: Option<Drag2d>,
    pub status_message: String,
    pub pointer_down: bool,
    pub dragging_3d: bool,
    pub last_pointer_pos: (f32, f32),
    pub window_size: (f32, f32),
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
    pub corner_radius_mm: f64,
    pub corner_tension: f64,
    pub corner_rule: CornerRule,
    pub open_accordion_idx: usize,
    pub accordion_open: [bool; 6],
    pub board_2d_pan: (f64, f64),
    pub board_2d_zoom: f64,
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
            actions: Arc::new(AtomicU64::new(0)),
            pending_ui_ops: Arc::new(Mutex::new(Vec::new())),
            smoke: false,
            smoke_done: Default::default(),
            usd_scene: None,
            project,
            preview_template: None,
            history: History::default(),
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
            selected_2d_point: None,
            selected_2d_edge: None,
            hovered_2d: None,
            drag_2d: None,
            status_message: "Ready".to_string(),
            pointer_down: false,
            dragging_3d: false,
            last_pointer_pos: (0.0, 0.0),
            window_size: (1600.0, 980.0),
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
            corner_radius_mm: 10.0,
            corner_tension: CIRCLE_TENSION,
            corner_rule: CornerRule::Smooth,
            open_accordion_idx: 0,
            accordion_open: [true, false, true, false, false, false],
            board_2d_pan: (0.0, 0.0),
            board_2d_zoom: 1.0,
        }
    }

    pub fn active_template(&self) -> &GarmentTemplate {
        self.preview_template
            .as_ref()
            .unwrap_or(&self.project.template)
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
        let mut snapshot = self.project.clone();
        snapshot.state = Some(self.scene.state.clone());
        snapshot.simulation = Some(self.settings.clone());
        match snapshot.save(&path) {
            Ok(_) => {
                self.project = snapshot;
                self.status_message = format!("Saved {}", path.display());
            }
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
    let actions = state.actions.swap(0, Ordering::Relaxed);
    let mut prev_piece = state.selected_piece_index;

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
    if actions & ACT_RUN_ACC_0 != 0 {
        state.accordion_open[0] = !state.accordion_open[0];
        state.open_accordion_idx = 0;
    }
    if actions & ACT_RUN_ACC_1 != 0 {
        state.accordion_open[1] = !state.accordion_open[1];
        state.open_accordion_idx = 1;
    }
    if actions & ACT_RUN_ACC_2 != 0 {
        state.accordion_open[2] = !state.accordion_open[2];
        state.open_accordion_idx = 2;
    }
    if actions & ACT_RUN_ACC_3 != 0 {
        state.accordion_open[3] = !state.accordion_open[3];
        state.open_accordion_idx = 3;
    }
    if actions & ACT_RUN_ACC_4 != 0 {
        state.accordion_open[4] = !state.accordion_open[4];
        state.open_accordion_idx = 4;
    }
    if actions & ACT_RUN_ACC_5 != 0 {
        state.accordion_open[5] = !state.accordion_open[5];
        state.open_accordion_idx = 5;
    }
    if actions & ACT_RUN_ACCORDION_TOGGLE != 0
        && (actions
            & (ACT_RUN_ACC_0
                | ACT_RUN_ACC_1
                | ACT_RUN_ACC_2
                | ACT_RUN_ACC_3
                | ACT_RUN_ACC_4
                | ACT_RUN_ACC_5)
            == 0)
    {
        state.open_accordion_idx = (state.open_accordion_idx + 1) % 6;
        state.accordion_open[state.open_accordion_idx] =
            !state.accordion_open[state.open_accordion_idx];
    }
    if actions & ACT_RUN_PIECE_0 != 0 && !state.project.template.panels.is_empty() {
        state.selected_piece_index = 0;
    }
    if actions & ACT_RUN_PIECE_1 != 0 && state.project.template.panels.len() > 1 {
        state.selected_piece_index = 1;
    }
    if actions & ACT_RUN_PIECE_2 != 0 && state.project.template.panels.len() > 2 {
        state.selected_piece_index = 2;
    }
    if actions & ACT_RUN_PIECE_3 != 0 && state.project.template.panels.len() > 3 {
        state.selected_piece_index = 3;
    }
    if actions & ACT_RUN_2D_ZOOM_IN != 0 {
        state.board_2d_zoom = (state.board_2d_zoom * 1.25).clamp(0.25, 10.0);
    }
    if actions & ACT_RUN_2D_ZOOM_OUT != 0 {
        state.board_2d_zoom = (state.board_2d_zoom / 1.25).clamp(0.25, 10.0);
    }
    if actions & ACT_RUN_2D_FIT != 0 {
        state.board_2d_pan = (0.0, 0.0);
        state.board_2d_zoom = 1.0;
    }
    if actions & ACT_RUN_2D_NEXT_PIECE != 0 {
        let count = state.project.template.panels.len().max(1);
        state.selected_piece_index = (state.selected_piece_index + 1) % count;
        if let Some(p) = state
            .project
            .template
            .panels
            .get(state.selected_piece_index)
        {
            state.status_message = format!("Selected piece: {} ({})", p.name, p.id);
        }
    }
    if state.selected_piece_index != prev_piece {
        prev_piece = state.selected_piece_index;
        let _ = prev_piece;
        state.selected_2d_point = None;
        state.selected_2d_edge = None;
    }

    let ui_ops: Vec<RunnerUiOp> = state
        .pending_ui_ops
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default();
    let had_ui_ops = !ui_ops.is_empty();
    for op in ui_ops {
        apply_runner_ui_op(state, op);
    }

    if actions != 0 || had_ui_ops || (had_result && !state.playing) {
        Invalidation::Structure
    } else if had_result {
        Invalidation::Paint
    } else {
        Invalidation::Clean
    }
}

fn queue_action(actions: &Arc<AtomicU64>, action: u64) -> impl Fn() + Send + Sync + 'static {
    let actions = Arc::clone(actions);
    move || {
        actions.fetch_or(action, Ordering::Relaxed);
    }
}

fn queue_ui_op(
    ops: &Arc<Mutex<Vec<RunnerUiOp>>>,
    op: RunnerUiOp,
) -> impl Fn() + Send + Sync + 'static {
    let ops = Arc::clone(ops);
    move || {
        if let Ok(mut guard) = ops.lock() {
            guard.push(op.clone());
        }
    }
}

fn apply_runner_ui_op(state: &mut CssRunnerState, op: RunnerUiOp) {
    match op {
        RunnerUiOp::SetCornerMode(radius) => {
            state.corner_mode_radius = radius;
            state.status_message = if radius {
                "Corner mode: Radius (circular fillet)".into()
            } else {
                format!("Corner mode: Shape (tension {:.2})", state.corner_tension)
            };
        }
        RunnerUiOp::SetCornerRule(rule) => {
            state.corner_rule = rule;
            state.status_message = format!("Continuity rule: {:?}", rule);
        }
        RunnerUiOp::AdjustCornerRadiusMm(delta) => {
            state.corner_radius_mm = (state.corner_radius_mm + delta).clamp(1.0, 250.0);
            state.status_message = format!("Corner radius: {:.1} mm", state.corner_radius_mm);
        }
        RunnerUiOp::AdjustCornerTension(delta) => {
            state.corner_tension = (state.corner_tension + delta).clamp(0.05, 1.5);
            state.corner_mode_radius = false;
            state.status_message = format!("Corner tension: {:.2}", state.corner_tension);
        }
        RunnerUiOp::ApplyCornerRoundSelected => {
            round_selected_corner(state);
        }
        RunnerUiOp::ToggleSelectedCurve(curved) => {
            toggle_selected_curve(state, curved);
        }
        RunnerUiOp::Undo2d => {
            undo_redo_2d(state, false);
        }
        RunnerUiOp::Redo2d => {
            undo_redo_2d(state, true);
        }
    }
}

/// Exact CSS layout rects for `#viewport-2d`'s `.canvas-area` and square `#svg-2d-board`.
pub fn compute_2d_viewport_rects(state: &CssRunnerState) -> (Rect2d, Rect2d) {
    let win_w = state.window_size.0.max(100.0);
    let win_h = state.window_size.1.max(100.0);
    let body_y = 40.0_f32;
    let body_h = (win_h - 40.0 - 26.0).max(1.0);

    let left_w = 48.0_f32
        + if state.left_drawer_open && !state.clean_view {
            240.0
        } else {
            0.0
        };
    let right_w = if state.clean_view {
        0.0_f32
    } else {
        48.0 + 300.0
    };
    let stage_w = (win_w - left_w - right_w).max(1.0);

    let (vp2d_x, vp2d_w) = match state.layout_mode {
        ViewLayoutMode::TwoDOnly => (left_w, stage_w),
        ViewLayoutMode::ThreeDOnly => return (Rect2d::default(), Rect2d::default()),
        ViewLayoutMode::Split => {
            let half = stage_w * 0.5;
            (left_w + half + 1.0, (half - 1.0).max(1.0))
        }
    };

    // `.viewport-header` (32px) + `.cad-2d-bar` (28px)
    let top_chrome_h = 32.0_f32 + 28.0_f32;
    let canvas_rect = Rect2d {
        x: vp2d_x,
        y: body_y + top_chrome_h,
        w: vp2d_w,
        h: (body_h - top_chrome_h).max(1.0),
    };
    let sq = canvas_rect.w.min(canvas_rect.h).floor().max(1.0);
    let svg_rect = Rect2d {
        x: canvas_rect.x + (canvas_rect.w - sq) * 0.5,
        y: canvas_rect.y + (canvas_rect.h - sq) * 0.5,
        w: sq,
        h: sq,
    };
    (canvas_rect, svg_rect)
}

/// Exact CSS layout rect for `#viewport-3d`'s `.canvas-area` (for 3D camera orbit gating).
pub fn viewport_3d_rect(state: &CssRunnerState) -> Rect2d {
    let win_w = state.window_size.0.max(100.0);
    let win_h = state.window_size.1.max(100.0);
    let body_y = 40.0_f32;
    let body_h = (win_h - 40.0 - 26.0).max(1.0);

    let left_w = 48.0_f32
        + if state.left_drawer_open && !state.clean_view {
            240.0
        } else {
            0.0
        };
    let right_w = if state.clean_view {
        0.0_f32
    } else {
        48.0 + 300.0
    };
    let stage_w = (win_w - left_w - right_w).max(1.0);

    let vp3d_w = match state.layout_mode {
        ViewLayoutMode::TwoDOnly => return Rect2d::default(),
        ViewLayoutMode::ThreeDOnly => stage_w,
        ViewLayoutMode::Split => stage_w * 0.5,
    };

    Rect2d {
        x: left_w,
        y: body_y + 32.0,
        w: vp3d_w,
        h: (body_h - 32.0).max(1.0),
    }
}

fn panel_raw_points(panel: &Panel) -> Vec<DVec2> {
    if let Some(ref poly) = panel.polygon
        && !poly.points.is_empty()
    {
        return poly.points.clone();
    }
    let w = panel.pattern.width.max(0.1);
    let h = panel.pattern.height.max(0.1);
    vec![
        DVec2::ZERO,
        DVec2::new(w, 0.0),
        DVec2::new(w, h),
        DVec2::new(0.0, h),
    ]
}

fn panel_to_board_pt(template: &GarmentTemplate, idx: usize, local: DVec2) -> DVec2 {
    let origin = authoring::layout(template, idx);
    let rot = template.panels[idx].layout_rotation_degrees;
    origin + authoring::rotate_2d(local, rot)
}

fn board_to_panel_pt(template: &GarmentTemplate, idx: usize, board: DVec2) -> DVec2 {
    let origin = authoring::layout(template, idx);
    let rot = template.panels[idx].layout_rotation_degrees;
    authoring::rotate_2d(board - origin, -rot)
}

/// Square 2D board viewBox `(min_x_m, min_y_m, span_m)` derived from `template` and pan/zoom.
pub fn compute_2d_view_box_m(state: &CssRunnerState) -> (f64, f64, f64) {
    let template = &state.project.template;
    let mut g_min_x = f64::INFINITY;
    let mut g_max_x = f64::NEG_INFINITY;
    let mut g_min_y = f64::INFINITY;
    let mut g_max_y = f64::NEG_INFINITY;

    for (idx, panel) in template.panels.iter().enumerate() {
        for pt in panel_raw_points(panel) {
            let b = panel_to_board_pt(template, idx, pt);
            g_min_x = g_min_x.min(b.x);
            g_max_x = g_max_x.max(b.x);
            g_min_y = g_min_y.min(b.y);
            g_max_y = g_max_y.max(b.y);
        }
        if let Some(ref poly) = panel.polygon {
            for [c1, c2] in poly.curves.values() {
                for c in [c1, c2] {
                    let b = panel_to_board_pt(template, idx, *c);
                    g_min_x = g_min_x.min(b.x);
                    g_max_x = g_max_x.max(b.x);
                    g_min_y = g_min_y.min(b.y);
                    g_max_y = g_max_y.max(b.y);
                }
            }
        }
    }

    if !g_min_x.is_finite() {
        g_min_x = 0.0;
        g_max_x = 0.6;
        g_min_y = 0.0;
        g_max_y = 0.6;
    }

    let span_x = (g_max_x - g_min_x).max(0.2);
    let span_y = (g_max_y - g_min_y).max(0.2);
    let max_span = span_x.max(span_y).max(0.4);
    let center_x = (g_min_x + g_max_x) * 0.5;
    let center_y = (g_min_y + g_max_y) * 0.5;
    let margin = (max_span * 0.12).max(0.06);
    let half_span = max_span * 0.5 + margin;

    let zoom = state.board_2d_zoom.clamp(0.25, 10.0);
    let view_span = (half_span * 2.0) / zoom;
    let vb_min_x = center_x + state.board_2d_pan.0 - view_span * 0.5;
    let vb_min_y = center_y + state.board_2d_pan.1 - view_span * 0.5;
    (vb_min_x, vb_min_y, view_span)
}

pub fn screen_to_board_m(state: &CssRunnerState, px: f32, py: f32) -> Option<DVec2> {
    let (_canvas, svg_rect) = compute_2d_viewport_rects(state);
    if !svg_rect.contains(px, py) {
        return None;
    }
    let (vb_x, vb_y, span) = compute_2d_view_box_m(state);
    let u = ((px - svg_rect.x) / svg_rect.w) as f64;
    let v = ((py - svg_rect.y) / svg_rect.h) as f64;
    Some(DVec2::new(vb_x + u * span, vb_y + v * span))
}

fn dist_point_to_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= 1e-18 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn point_in_poly_2d(p: DVec2, ring: &[DVec2]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[j];
        if ((a.y > p.y) != (b.y > p.y)) && (p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn sample_polygon_boundary(poly: &PolygonPattern) -> Vec<DVec2> {
    let mut ring = Vec::new();
    for edge_idx in 0..poly.edges.len() {
        if let Ok(pts) = poly.edge_points(edge_idx)
            && pts.len() >= 2
        {
            ring.extend_from_slice(&pts[..pts.len() - 1]);
        }
    }
    ring
}

fn panel_edge_length_m(panel: &Panel, edge_name: &str) -> Option<f64> {
    let poly = panel.polygon.as_ref()?;
    let idx = poly.edges.iter().position(|e| e == edge_name)?;
    let pts = poly.edge_points(idx).ok()?;
    Some(pts.windows(2).map(|pair| pair[0].distance(pair[1])).sum())
}

fn dashed_path_d(pts: &[(f64, f64)], dash: f64, gap: f64) -> String {
    if pts.len() < 2 || dash <= 1e-6 {
        return String::new();
    }
    let period = dash + gap.max(0.0);
    let mut d = String::new();
    let mut draw = true;
    let mut rem = dash;
    for pair in pts.windows(2) {
        let mut p0 = DVec2::new(pair[0].0, pair[0].1);
        let p1 = DVec2::new(pair[1].0, pair[1].1);
        let mut left = p0.distance(p1);
        if left <= 1e-9 {
            continue;
        }
        let dir = (p1 - p0) / left;
        while left > 1e-9 {
            let step = rem.min(left);
            let next = p0 + dir * step;
            if draw {
                d.push_str(&format!(
                    "M {:.2},{:.2} L {:.2},{:.2} ",
                    p0.x, p0.y, next.x, next.y
                ));
            }
            p0 = next;
            left -= step;
            rem -= step;
            if rem <= 1e-9 {
                draw = !draw;
                rem = if draw { dash } else { gap.max(period * 0.1) };
            }
        }
    }
    d
}

fn active_fillet_preview(state: &CssRunnerState) -> Option<(usize, String, FilletPreview, f64)> {
    if state.active_2d_tool != "Corner" {
        return None;
    }
    let template = state.active_template();
    let (panel_idx, point_idx) = match &state.drag_2d {
        Some(Drag2d::CornerRadius {
            panel_id,
            edge_name,
            ..
        }) => {
            let pidx = template.panels.iter().position(|p| &p.id == panel_id)?;
            let eidx = template.panels[pidx]
                .polygon
                .as_ref()?
                .edges
                .iter()
                .position(|e| e == edge_name)?;
            (pidx, eidx)
        }
        _ => match &state.hovered_2d {
            Some(Hover2dTarget::CornerPreviewHandle {
                panel_idx,
                edge_name,
            }) => {
                let p = template.panels.get(*panel_idx)?;
                let eidx = p
                    .polygon
                    .as_ref()?
                    .edges
                    .iter()
                    .position(|e| e == edge_name)?;
                (*panel_idx, eidx)
            }
            Some(Hover2dTarget::Point {
                panel_idx,
                point_idx,
            }) => (*panel_idx, *point_idx),
            _ => {
                let (pid, pt) = state.selected_2d_point.as_ref()?;
                let pidx = template.panels.iter().position(|p| &p.id == pid)?;
                (pidx, *pt)
            }
        },
    };
    let panel = template.panels.get(panel_idx)?;
    let poly = panel.polygon.as_ref()?;
    let edge_name = poly.edges.get(point_idx)?;
    let max_r = max_fillet_radius(panel, edge_name).ok()?;
    if max_r <= 0.0005 {
        return None;
    }
    let eff_r = (state.corner_radius_mm / 1000.0).clamp(0.0005, max_r * 0.98);
    let tension = if state.corner_mode_radius {
        CIRCLE_TENSION
    } else {
        state.corner_tension
    };
    let preview = validate_fillet_preview(template, &panel.id, edge_name, eff_r, tension).ok()?;
    Some((panel_idx, edge_name.clone(), preview, eff_r))
}

fn fillet_drag_handle_local(preview: &FilletPreview) -> DVec2 {
    curves::cubic(
        [
            preview.start,
            preview.control_1,
            preview.control_2,
            preview.end,
        ],
        0.5,
    )
}

fn panel_order_selected_first(len: usize, selected: usize) -> Vec<usize> {
    let mut order = Vec::with_capacity(len);
    if selected < len {
        order.push(selected);
    }
    for i in 0..len {
        if i != selected {
            order.push(i);
        }
    }
    order
}

fn hit_test_2d(state: &CssRunnerState, board_m: DVec2) -> Option<Hover2dTarget> {
    let (_canvas, svg_rect) = compute_2d_viewport_rects(state);
    let (_vbx, _vby, view_span) = compute_2d_view_box_m(state);
    let px_m = view_span / (svg_rect.w.max(1.0) as f64);
    let hit_r = 10.0 * px_m;
    let template = state.active_template();

    // 1. Live Corner Fillet radius handle
    if let Some((panel_idx, edge_name, preview, _eff_r)) = active_fillet_preview(state) {
        let knob_b = panel_to_board_pt(template, panel_idx, fillet_drag_handle_local(&preview));
        if board_m.distance(knob_b) <= hit_r * 1.35 {
            return Some(Hover2dTarget::CornerPreviewHandle {
                panel_idx,
                edge_name,
            });
        }
    }

    let order = panel_order_selected_first(template.panels.len(), state.selected_piece_index);

    // 2. Bézier tangent handles across panels (selected panel checked first)
    for &panel_idx in &order {
        let panel = &template.panels[panel_idx];
        if let Some(ref poly) = panel.polygon {
            for (edge_idx, edge_name) in poly.edges.iter().enumerate() {
                if let Some([c1, c2]) = poly.curves.get(edge_name) {
                    let c1_b = panel_to_board_pt(template, panel_idx, *c1);
                    let c2_b = panel_to_board_pt(template, panel_idx, *c2);
                    if board_m.distance(c1_b) <= hit_r * 1.25 {
                        return Some(Hover2dTarget::BezierHandle {
                            panel_idx,
                            edge_idx,
                            handle_index: 0,
                        });
                    }
                    if board_m.distance(c2_b) <= hit_r * 1.25 {
                        return Some(Hover2dTarget::BezierHandle {
                            panel_idx,
                            edge_idx,
                            handle_index: 1,
                        });
                    }
                }
            }
        }
    }

    // 3. Contour points across panels
    for &panel_idx in &order {
        let panel = &template.panels[panel_idx];
        let pts = panel_raw_points(panel);
        let mut best: Option<(usize, f64)> = None;
        for (pt_idx, pt) in pts.iter().enumerate() {
            let b = panel_to_board_pt(template, panel_idx, *pt);
            let d = board_m.distance(b);
            if d <= hit_r && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((pt_idx, d));
            }
        }
        if let Some((point_idx, _)) = best {
            return Some(Hover2dTarget::Point {
                panel_idx,
                point_idx,
            });
        }
    }

    // 4. Contour edges across panels
    let edge_hit_r = 8.0 * px_m;
    for &panel_idx in &order {
        let panel = &template.panels[panel_idx];
        if let Some(ref poly) = panel.polygon {
            let mut best: Option<(usize, f64)> = None;
            for edge_idx in 0..poly.edges.len() {
                if let Ok(sampled) = poly.edge_points(edge_idx) {
                    let mut min_d = f64::INFINITY;
                    for pair in sampled.windows(2) {
                        let a = panel_to_board_pt(template, panel_idx, pair[0]);
                        let b = panel_to_board_pt(template, panel_idx, pair[1]);
                        min_d = min_d.min(dist_point_to_segment(board_m, a, b));
                    }
                    if min_d <= edge_hit_r && best.is_none_or(|(_, bd)| min_d < bd) {
                        best = Some((edge_idx, min_d));
                    }
                }
            }
            if let Some((edge_idx, _)) = best {
                return Some(Hover2dTarget::Edge {
                    panel_idx,
                    edge_idx,
                });
            }
        }
    }

    // 5. Panel interiors
    for &panel_idx in &order {
        let panel = &template.panels[panel_idx];
        let ring: Vec<DVec2> = if let Some(ref poly) = panel.polygon {
            sample_polygon_boundary(poly)
                .into_iter()
                .map(|p| panel_to_board_pt(template, panel_idx, p))
                .collect()
        } else {
            panel_raw_points(panel)
                .into_iter()
                .map(|p| panel_to_board_pt(template, panel_idx, p))
                .collect()
        };
        if point_in_poly_2d(board_m, &ring) {
            return Some(Hover2dTarget::Panel(panel_idx));
        }
    }

    None
}

fn sanitize_2d_subselection(state: &mut CssRunnerState) {
    if state.selected_piece_index >= state.project.template.panels.len() {
        state.selected_piece_index = 0;
    }
    if let Some((ref pid, pt_idx)) = state.selected_2d_point {
        let valid = state
            .project
            .template
            .panels
            .iter()
            .find(|p| &p.id == pid)
            .and_then(|p| p.polygon.as_ref())
            .is_some_and(|poly| pt_idx < poly.points.len());
        if !valid {
            state.selected_2d_point = None;
        }
    }
    if let Some((ref pid, ref edge_name)) = state.selected_2d_edge {
        let valid = state
            .project
            .template
            .panels
            .iter()
            .find(|p| &p.id == pid)
            .and_then(|p| p.polygon.as_ref())
            .is_some_and(|poly| poly.edges.iter().any(|e| e == edge_name));
        if !valid {
            state.selected_2d_edge = None;
        }
    }
}

fn install_authoring_edit(
    state: &mut CssRunnerState,
    edit: authoring::Edit,
    record_old: Option<GarmentTemplate>,
    msg: String,
) {
    if let Some(old) = record_old {
        state.history.record(old);
    }
    let next_garment = Arc::new(edit.garment.clone());
    let next_state = if edit.reset_simulation {
        edit.state_after(&state.scene.garment, &state.scene.state)
    } else {
        state.scene.state.clone()
    };

    if let Some(ref sel_id) = edit.selected
        && let Some(idx) = edit.template.panels.iter().position(|p| &p.id == sel_id)
    {
        state.selected_piece_index = idx;
    }
    if let Some(np) = edit.new_point
        && let Some(p) = edit.template.panels.get(state.selected_piece_index)
    {
        state.selected_2d_point = Some((p.id.clone(), np));
        state.selected_2d_edge = None;
    }

    state.project.template = edit.template;
    if edit.reset_simulation {
        state.project.state = None;
        state.project.mesh_signature = None;
        state.generation = state.generation.wrapping_add(1);
        state.worker_pending = false;
        state.scene = FittingScene {
            garment: Arc::clone(&next_garment),
            collider: Arc::clone(&state.scene.collider),
            state: next_state.clone(),
            quasistatic: state.scene.quasistatic,
            next_dt: state.scene.next_dt,
        };
        state.initial_scene = FittingScene {
            garment: next_garment,
            collider: Arc::clone(&state.scene.collider),
            state: state.scene.garment.initial_state.clone(),
            quasistatic: state.scene.quasistatic,
            next_dt: state.scene.next_dt,
        };
        if let Some(ref usd) = state.usd_scene {
            let _ = marvelous_cloth::usd_scene::update_panel_viewport_mesh(
                usd,
                &state.scene.garment,
                &state.scene.state,
            );
        }
        state.send_control_state(false);
    }

    sanitize_2d_subselection(state);
    state.status_message = msg;
}

pub fn commit_authoring_command(
    state: &mut CssRunnerState,
    cmd: Command,
    msg: impl Into<String>,
) -> bool {
    state.preview_template = None;
    let before = state.project.template.clone();
    match authoring::apply(&before, cmd) {
        Ok(edit) => {
            install_authoring_edit(state, edit, Some(before), msg.into());
            true
        }
        Err(err) => {
            state.status_message = format!("Edit rejected: {err}");
            false
        }
    }
}

pub fn undo_redo_2d(state: &mut CssRunnerState, redo: bool) -> bool {
    state.preview_template = None;
    match state.history.restore(&state.project.template, redo) {
        Ok(edit) => {
            let label = if redo {
                "Redid 2D edit"
            } else {
                "Undid 2D edit"
            };
            install_authoring_edit(state, edit, None, label.into());
            true
        }
        Err(err) => {
            state.status_message = err;
            false
        }
    }
}

fn round_corner_by_name(
    state: &mut CssRunnerState,
    panel_id: &str,
    edge_name: &str,
    max_radius_m: f64,
) -> bool {
    let eff_r = (state.corner_radius_mm / 1000.0).clamp(0.0005, max_radius_m * 0.98);
    let tension = if state.corner_mode_radius {
        CIRCLE_TENSION
    } else {
        state.corner_tension
    };
    let ok = commit_authoring_command(
        state,
        Command::RoundCornerShape {
            panel: panel_id.to_owned(),
            edge: edge_name.to_owned(),
            radius_m: eff_r,
            tension,
            rule: state.corner_rule,
        },
        format!(
            "Rounded corner on {edge_name} (R={:.1} mm, T={tension:.2})",
            eff_r * 1000.0
        ),
    );
    if ok {
        // Select the newly created fillet curve edge for immediate Bézier inspection
        if let Some(p) = state
            .project
            .template
            .panels
            .iter()
            .find(|p| p.id == panel_id)
            && let Some(ref poly) = p.polygon
            && let Some(idx) = poly.edges.iter().position(|e| e == edge_name)
        {
            let fillet_idx = (idx + poly.edges.len() - 1) % poly.edges.len();
            state.selected_2d_edge = Some((panel_id.to_owned(), poly.edges[fillet_idx].clone()));
            state.selected_2d_point = None;
        }
    }
    ok
}

fn round_selected_corner(state: &mut CssRunnerState) -> bool {
    let target = if let Some((ref pid, pt_idx)) = state.selected_2d_point {
        state
            .project
            .template
            .panels
            .iter()
            .find(|p| &p.id == pid)
            .and_then(|p| {
                p.polygon
                    .as_ref()
                    .and_then(|poly| poly.edges.get(pt_idx))
                    .map(|e| (pid.clone(), e.clone(), p))
            })
    } else if let Some((ref pid, ref ename)) = state.selected_2d_edge {
        state
            .project
            .template
            .panels
            .iter()
            .find(|p| &p.id == pid)
            .map(|p| (pid.clone(), ename.clone(), p))
    } else {
        None
    };

    let Some((pid, ename, panel)) = target else {
        state.status_message = "Select a corner point first to round it".into();
        return false;
    };
    match max_fillet_radius(panel, &ename) {
        Ok(max_r) if max_r > 0.0005 => round_corner_by_name(state, &pid, &ename, max_r),
        Ok(_) => {
            state.status_message = "Incident boundaries too short to round".into();
            false
        }
        Err(err) => {
            state.status_message = format!("Cannot round corner: {err}");
            false
        }
    }
}

fn toggle_selected_curve(state: &mut CssRunnerState, curved: bool) -> bool {
    let target = if let Some((ref pid, pt_idx)) = state.selected_2d_point {
        Some((pid.clone(), pt_idx))
    } else if let Some((ref pid, ref ename)) = state.selected_2d_edge {
        state
            .project
            .template
            .panels
            .iter()
            .find(|p| &p.id == pid)
            .and_then(|p| p.polygon.as_ref())
            .and_then(|poly| poly.edges.iter().position(|e| e == ename))
            .map(|idx| (pid.clone(), idx))
    } else {
        None
    };
    let Some((panel, anchor)) = target else {
        state.status_message = "Select a point or boundary to toggle Bézier curves".into();
        return false;
    };
    commit_authoring_command(
        state,
        Command::CurvePoint {
            panel,
            anchor,
            curved,
        },
        if curved {
            "Converted incident boundaries to cubic Bézier".to_string()
        } else {
            "Straightened incident boundaries".to_string()
        },
    )
}

fn update_preview_point(
    state: &mut CssRunnerState,
    panel_id: &str,
    point_idx: usize,
    local_pos: DVec2,
) {
    let mut draft = state.project.template.clone();
    if let Some(panel) = draft.panels.iter_mut().find(|p| p.id == panel_id)
        && let Some(ref mut poly) = panel.polygon
        && point_idx < poly.points.len()
    {
        let n = poly.points.len();
        let old = poly.points[point_idx];
        let delta = local_pos - old;
        poly.points[point_idx] = local_pos;
        if let Some(controls) = poly.curves.get_mut(&poly.edges[point_idx]) {
            controls[0] += delta;
        }
        let prev = (point_idx + n - 1) % n;
        if let Some(controls) = poly.curves.get_mut(&poly.edges[prev]) {
            controls[1] += delta;
        }
        state.preview_template = Some(draft);
    }
}

fn update_preview_bezier_handle(
    state: &mut CssRunnerState,
    panel_id: &str,
    edge_name: &str,
    handle_index: usize,
    position: DVec2,
) {
    let mut draft = state.project.template.clone();
    if let Some(panel) = draft.panels.iter_mut().find(|p| p.id == panel_id)
        && let Some(ref mut poly) = panel.polygon
        && let Some(edge_idx) = poly.edges.iter().position(|e| e == edge_name)
    {
        let n = poly.points.len();
        let anchor_idx = if handle_index == 0 {
            edge_idx
        } else {
            (edge_idx + 1) % n
        };
        let anchor = poly.points[anchor_idx];
        let vec = position - anchor;
        let len = vec.length();
        if len > 1e-9 && len.is_finite() {
            let dir = vec / len;
            let end_idx = (edge_idx + 1) % n;
            let p_start = poly.points[edge_idx];
            let p_end = poly.points[end_idx];
            let controls = poly.curves.entry(edge_name.to_owned()).or_insert_with(|| {
                [
                    p_start + (p_end - p_start) / 3.0,
                    p_start + (p_end - p_start) * (2.0 / 3.0),
                ]
            });
            controls[handle_index] = position;

            let neighbor_idx = if handle_index == 0 {
                (edge_idx + n - 1) % n
            } else {
                (edge_idx + 1) % n
            };
            let neighbor_edge = poly.edges[neighbor_idx].clone();
            match state.corner_rule {
                CornerRule::Sharp => {}
                CornerRule::Smooth => {
                    if let Some(nc) = poly.curves.get_mut(&neighbor_edge) {
                        let slot = if handle_index == 0 { 1 } else { 0 };
                        let opp_len = (nc[slot] - anchor).length().max(1e-6);
                        nc[slot] = anchor - dir * opp_len;
                    }
                }
                CornerRule::Symmetric => {
                    let neighbor_end = poly.points[(neighbor_idx + 1) % n];
                    let neighbor_start = poly.points[neighbor_idx];
                    let nc = poly.curves.entry(neighbor_edge).or_insert_with(|| {
                        [
                            neighbor_start + (neighbor_end - neighbor_start) / 3.0,
                            neighbor_start + (neighbor_end - neighbor_start) * (2.0 / 3.0),
                        ]
                    });
                    let slot = if handle_index == 0 { 1 } else { 0 };
                    nc[slot] = anchor - dir * len;
                }
            }
            state.preview_template = Some(draft);
        }
    }
}

pub fn handle_runner_event(state: &mut CssRunnerState, event: &EngineEvent) -> bool {
    match event {
        EngineEvent::ViewportChanged(vp) => {
            let new_size = (vp.width.max(1) as f32, vp.height.max(1) as f32);
            if state.window_size != new_size {
                state.window_size = new_size;
                return true;
            }
            false
        }
        EngineEvent::Key(k) => {
            if k.state == cssimpler::renderer::ButtonState::Pressed {
                let ctrl = k.modifiers.control || k.modifiers.super_key;
                if let cssimpler::renderer::KeyIdentity::Character(ref ch) = k.logical_key {
                    if ch == " " {
                        state.playing = !state.playing;
                        state.send_control_state(false);
                        return true;
                    }
                    if ch == "k" && ctrl {
                        state.command_palette_open = !state.command_palette_open;
                        return true;
                    }
                    if ch == "s" && ctrl {
                        state.save_project();
                        return true;
                    }
                    if (ch == "z" || ch == "Z") && ctrl {
                        if k.modifiers.shift {
                            undo_redo_2d(state, true);
                        } else {
                            undo_redo_2d(state, false);
                        }
                        return true;
                    }
                    if (ch == "y" || ch == "Y") && ctrl {
                        undo_redo_2d(state, true);
                        return true;
                    }
                    if !ctrl {
                        if ch == "z" || ch == "Z" || ch == "a" || ch == "A" {
                            state.active_2d_tool = "Edit".to_string();
                            return true;
                        }
                        if ch == "c" || ch == "C" {
                            state.active_2d_tool = "Corner".to_string();
                            return true;
                        }
                        if ch == "x" || ch == "X" {
                            state.active_2d_tool = "Split".to_string();
                            return true;
                        }
                    }
                    if ch == "]" {
                        let count = state.project.template.panels.len().max(1);
                        state.selected_piece_index = (state.selected_piece_index + 1) % count;
                        state.selected_2d_point = None;
                        state.selected_2d_edge = None;
                        if let Some(p) = state
                            .project
                            .template
                            .panels
                            .get(state.selected_piece_index)
                        {
                            state.status_message = format!("Selected piece: {} ({})", p.name, p.id);
                        }
                        return true;
                    }
                    if ch == "[" {
                        let count = state.project.template.panels.len().max(1);
                        state.selected_piece_index =
                            (state.selected_piece_index + count - 1) % count;
                        state.selected_2d_point = None;
                        state.selected_2d_edge = None;
                        if let Some(p) = state
                            .project
                            .template
                            .panels
                            .get(state.selected_piece_index)
                        {
                            state.status_message = format!("Selected piece: {} ({})", p.name, p.id);
                        }
                        return true;
                    }
                    if ch == "+" || ch == "=" {
                        state.board_2d_zoom = (state.board_2d_zoom * 1.25).clamp(0.25, 10.0);
                        return true;
                    }
                    if ch == "-" || ch == "_" {
                        state.board_2d_zoom = (state.board_2d_zoom / 1.25).clamp(0.25, 10.0);
                        return true;
                    }
                    if ch == "0" {
                        state.board_2d_pan = (0.0, 0.0);
                        state.board_2d_zoom = 1.0;
                        return true;
                    }
                }
                if let cssimpler::renderer::KeyIdentity::Named(ref name) = k.logical_key {
                    if name == "Escape" {
                        if state.command_palette_open || state.solver_menu_open {
                            state.command_palette_open = false;
                            state.solver_menu_open = false;
                            return true;
                        }
                        if state.drag_2d.is_some()
                            || state.selected_2d_point.is_some()
                            || state.selected_2d_edge.is_some()
                        {
                            state.drag_2d = None;
                            state.preview_template = None;
                            state.selected_2d_point = None;
                            state.selected_2d_edge = None;
                            return true;
                        }
                    }
                    if (name == "Delete" || name == "Backspace")
                        && let Some((panel, point)) = state.selected_2d_point.clone()
                    {
                        commit_authoring_command(
                            state,
                            Command::RemovePoint { panel, point },
                            format!("Removed point #{point}"),
                        );
                        return true;
                    }
                    if name == "Enter" && state.active_2d_tool == "Corner" {
                        round_selected_corner(state);
                        return true;
                    }
                }
            }
            false
        }
        EngineEvent::PointerButton {
            button:
                cssimpler::renderer::PointerButton::Secondary
                | cssimpler::renderer::PointerButton::Middle,
            state: btn_state,
            ..
        } => {
            if state.command_palette_open || state.solver_menu_open {
                return false;
            }
            let (canvas_2d, _svg_2d) = compute_2d_viewport_rects(state);
            let (px, py) = state.last_pointer_pos;
            if *btn_state == cssimpler::renderer::ButtonState::Pressed && canvas_2d.contains(px, py)
            {
                state.pointer_down = true;
                state.drag_2d = Some(Drag2d::Pan {
                    start_pan: state.board_2d_pan,
                    start_pointer: (px, py),
                });
                return true;
            }
            if *btn_state == cssimpler::renderer::ButtonState::Released
                && matches!(state.drag_2d, Some(Drag2d::Pan { .. }))
            {
                state.pointer_down = false;
                state.drag_2d = None;
                return true;
            }
            false
        }
        EngineEvent::PointerButton {
            button: cssimpler::renderer::PointerButton::Primary,
            state: btn_state,
            ..
        } => {
            if state.command_palette_open || state.solver_menu_open {
                return false;
            }
            let was_down = state.pointer_down;
            state.pointer_down = *btn_state == cssimpler::renderer::ButtonState::Pressed;
            let (px, py) = state.last_pointer_pos;
            let (canvas_2d, _svg_2d) = compute_2d_viewport_rects(state);
            let rect_3d = viewport_3d_rect(state);

            if state.pointer_down && !was_down {
                if canvas_2d.contains(px, py) {
                    if let Some(board_m) = screen_to_board_m(state, px, py) {
                        let hit = hit_test_2d(state, board_m);
                        state.hovered_2d = hit.clone();
                        match hit {
                            Some(Hover2dTarget::CornerPreviewHandle {
                                panel_idx,
                                edge_name,
                            }) => {
                                if let Some(preview) =
                                    state.active_template().panels.get(panel_idx).and_then(|p| {
                                        let max_r = max_fillet_radius(p, &edge_name).ok()?;
                                        let eff_r = (state.corner_radius_mm / 1000.0)
                                            .clamp(0.0005, max_r * 0.98);
                                        let prev =
                                            preview_fillet(p, &edge_name, eff_r, CIRCLE_TENSION)
                                                .ok()?;
                                        Some((p.id.clone(), prev, max_r))
                                    })
                                {
                                    let (pid, prev, max_r) = preview;
                                    let half_turn = prev.turn_radians.abs() * 0.5;
                                    let denom = (half_turn.tan() * half_turn.sin()).max(1e-4);
                                    let corner_board_m = panel_to_board_pt(
                                        &state.project.template,
                                        panel_idx,
                                        prev.corner,
                                    );
                                    let bisector = authoring::rotate_2d(
                                        prev.bisector,
                                        state.project.template.panels[panel_idx]
                                            .layout_rotation_degrees,
                                    );
                                    state.selected_piece_index = panel_idx;
                                    state.drag_2d = Some(Drag2d::CornerRadius {
                                        panel_id: pid,
                                        edge_name,
                                        corner_board_m,
                                        bisector,
                                        denom,
                                        max_radius_m: max_r,
                                        start_pointer: (px, py),
                                        moved: false,
                                    });
                                    return true;
                                }
                            }
                            Some(Hover2dTarget::BezierHandle {
                                panel_idx,
                                edge_idx,
                                handle_index,
                            }) => {
                                if let Some(p) = state.project.template.panels.get(panel_idx)
                                    && let Some(ref poly) = p.polygon
                                    && let Some(edge_name) = poly.edges.get(edge_idx)
                                {
                                    let local = board_to_panel_pt(
                                        &state.project.template,
                                        panel_idx,
                                        board_m,
                                    );
                                    state.selected_piece_index = panel_idx;
                                    state.selected_2d_edge =
                                        Some((p.id.clone(), edge_name.clone()));
                                    state.selected_2d_point = None;
                                    state.drag_2d = Some(Drag2d::BezierHandle {
                                        panel_id: p.id.clone(),
                                        edge_name: edge_name.clone(),
                                        handle_index,
                                        start_pointer: (px, py),
                                        moved: false,
                                        last_local: local,
                                    });
                                    state.status_message = format!(
                                        "Editing Bézier handle {handle_index} on {edge_name}"
                                    );
                                    return true;
                                }
                            }
                            Some(Hover2dTarget::Point {
                                panel_idx,
                                point_idx,
                            }) => {
                                if let Some(p) = state.project.template.panels.get(panel_idx) {
                                    let pid = p.id.clone();
                                    let pname = p.name.clone();
                                    state.selected_piece_index = panel_idx;
                                    state.selected_2d_point = Some((pid.clone(), point_idx));
                                    state.selected_2d_edge = None;
                                    if state.active_2d_tool == "Corner" {
                                        if let Some(ref poly) = p.polygon
                                            && let Some(edge_name) = poly.edges.get(point_idx)
                                        {
                                            match max_fillet_radius(p, edge_name) {
                                                Ok(max_r) => {
                                                    let eff_r = (state.corner_radius_mm / 1000.0)
                                                        .clamp(0.0005, max_r * 0.98);
                                                    match validate_fillet_preview(
                                                        &state.project.template,
                                                        &pid,
                                                        edge_name,
                                                        eff_r,
                                                        if state.corner_mode_radius {
                                                            CIRCLE_TENSION
                                                        } else {
                                                            state.corner_tension
                                                        },
                                                    ) {
                                                        Ok(prev) => {
                                                            let half_turn =
                                                                prev.turn_radians.abs() * 0.5;
                                                            let denom = (half_turn.tan()
                                                                * half_turn.sin())
                                                            .max(1e-4);
                                                            let corner_board_m = panel_to_board_pt(
                                                                &state.project.template,
                                                                panel_idx,
                                                                prev.corner,
                                                            );
                                                            let bisector = authoring::rotate_2d(
                                                                prev.bisector,
                                                                p.layout_rotation_degrees,
                                                            );
                                                            state.drag_2d =
                                                                Some(Drag2d::CornerRadius {
                                                                    panel_id: pid,
                                                                    edge_name: edge_name.clone(),
                                                                    corner_board_m,
                                                                    bisector,
                                                                    denom,
                                                                    max_radius_m: max_r,
                                                                    start_pointer: (px, py),
                                                                    moved: false,
                                                                });
                                                            return true;
                                                        }
                                                        Err(err) => {
                                                            state.status_message = err;
                                                            return true;
                                                        }
                                                    }
                                                }
                                                Err(err) => {
                                                    state.status_message = err;
                                                    return true;
                                                }
                                            }
                                        }
                                    } else {
                                        state.drag_2d = Some(Drag2d::Point {
                                            panel_id: pid,
                                            point_idx,
                                            start_pointer: (px, py),
                                            moved: false,
                                        });
                                        state.status_message =
                                            format!("Selected corner #{point_idx} on {pname}");
                                        return true;
                                    }
                                }
                            }
                            Some(Hover2dTarget::Edge {
                                panel_idx,
                                edge_idx,
                            }) => {
                                if let Some(p) = state.project.template.panels.get(panel_idx)
                                    && let Some(ref poly) = p.polygon
                                    && let Some(edge_name) = poly.edges.get(edge_idx)
                                {
                                    let pid = p.id.clone();
                                    let ename = edge_name.clone();
                                    state.selected_piece_index = panel_idx;
                                    state.selected_2d_edge = Some((pid.clone(), ename.clone()));
                                    state.selected_2d_point = None;
                                    if state.active_2d_tool == "Split" {
                                        if let Ok(sampled) = poly.edge_points(edge_idx) {
                                            let local = board_to_panel_pt(
                                                &state.project.template,
                                                panel_idx,
                                                board_m,
                                            );
                                            let mut acc_s = 0.0_f64;
                                            let mut best_s = 0.0_f64;
                                            let mut best_d = f64::INFINITY;
                                            for pair in sampled.windows(2) {
                                                let seg = pair[1] - pair[0];
                                                let seg_l = seg.length();
                                                let t = if seg_l > 1e-12 {
                                                    ((local - pair[0]).dot(seg) / (seg_l * seg_l))
                                                        .clamp(0.0, 1.0)
                                                } else {
                                                    0.0
                                                };
                                                let proj = pair[0] + seg * t;
                                                let d = local.distance(proj);
                                                if d < best_d {
                                                    best_d = d;
                                                    best_s = acc_s + seg_l * t;
                                                }
                                                acc_s += seg_l;
                                            }
                                            if acc_s > 0.002 {
                                                let at_m = best_s.clamp(0.001, acc_s - 0.001);
                                                commit_authoring_command(
                                                    state,
                                                    Command::SplitBoundary {
                                                        panel: pid,
                                                        edge: ename.clone(),
                                                        at_m,
                                                    },
                                                    format!("Split boundary {ename}"),
                                                );
                                            }
                                        }
                                        return true;
                                    }
                                    state.status_message =
                                        format!("Selected edge {ename} on {}", p.name);
                                    return true;
                                }
                            }
                            Some(Hover2dTarget::Panel(panel_idx)) => {
                                state.selected_piece_index = panel_idx;
                                state.selected_2d_point = None;
                                state.selected_2d_edge = None;
                                state.drag_2d = Some(Drag2d::Pan {
                                    start_pan: state.board_2d_pan,
                                    start_pointer: (px, py),
                                });
                                if let Some(p) = state.project.template.panels.get(panel_idx) {
                                    state.status_message =
                                        format!("Selected piece: {} ({})", p.name, p.id);
                                }
                                return true;
                            }
                            None => {
                                state.selected_2d_point = None;
                                state.selected_2d_edge = None;
                                state.drag_2d = Some(Drag2d::Pan {
                                    start_pan: state.board_2d_pan,
                                    start_pointer: (px, py),
                                });
                                return true;
                            }
                        }
                    } else {
                        // Inside canvas_2d padding outside square svg_2d: pan only
                        state.drag_2d = Some(Drag2d::Pan {
                            start_pan: state.board_2d_pan,
                            start_pointer: (px, py),
                        });
                        return true;
                    }
                } else if rect_3d.contains(px, py) {
                    state.dragging_3d = true;
                }
            } else if !state.pointer_down {
                state.dragging_3d = false;
                if let Some(drag) = state.drag_2d.take() {
                    match drag {
                        Drag2d::Point {
                            panel_id,
                            point_idx,
                            moved,
                            ..
                        } => {
                            state.preview_template = None;
                            if moved
                                && let Some(board_m) = screen_to_board_m(state, px, py)
                                && let Some(panel_idx) = state
                                    .project
                                    .template
                                    .panels
                                    .iter()
                                    .position(|p| p.id == panel_id)
                            {
                                let local =
                                    board_to_panel_pt(&state.project.template, panel_idx, board_m);
                                commit_authoring_command(
                                    state,
                                    Command::Anchor {
                                        panel: panel_id,
                                        anchor: point_idx,
                                        position: local,
                                    },
                                    format!(
                                        "Moved point #{point_idx} to ({:.1}, {:.1}) mm",
                                        local.x * 1000.0,
                                        local.y * 1000.0
                                    ),
                                );
                            }
                            return true;
                        }
                        Drag2d::BezierHandle {
                            panel_id,
                            edge_name,
                            handle_index,
                            moved,
                            last_local,
                            ..
                        } => {
                            state.preview_template = None;
                            if moved {
                                commit_authoring_command(
                                    state,
                                    Command::AdjustCornerHandle {
                                        panel: panel_id,
                                        edge: edge_name.clone(),
                                        handle_index,
                                        position: last_local,
                                        rule: state.corner_rule,
                                    },
                                    format!("Adjusted Bézier handle on {edge_name}"),
                                );
                            }
                            return true;
                        }
                        Drag2d::CornerRadius {
                            panel_id,
                            edge_name,
                            max_radius_m,
                            ..
                        } => {
                            state.preview_template = None;
                            round_corner_by_name(state, &panel_id, &edge_name, max_radius_m);
                            return true;
                        }
                        Drag2d::Pan { .. } => {
                            return true;
                        }
                    }
                }
            }
            false
        }
        EngineEvent::PointerMoved { position, .. } => {
            let (px, py) = (position.x, position.y);
            let dx = px - state.last_pointer_pos.0;
            let dy = py - state.last_pointer_pos.1;
            state.last_pointer_pos = (px, py);

            if state.drag_2d.is_some() {
                let (_canvas, svg_rect) = compute_2d_viewport_rects(state);
                let (_vbx, _vby, view_span) = compute_2d_view_box_m(state);
                let px_m = view_span / (svg_rect.w.max(1.0) as f64);

                let drag = state.drag_2d.take().unwrap();
                match drag {
                    Drag2d::Pan {
                        start_pan,
                        start_pointer,
                    } => {
                        let dpx = (px - start_pointer.0) as f64;
                        let dpy = (py - start_pointer.1) as f64;
                        state.board_2d_pan.0 = start_pan.0 - dpx * px_m;
                        state.board_2d_pan.1 = start_pan.1 - dpy * px_m;
                        state.drag_2d = Some(Drag2d::Pan {
                            start_pan,
                            start_pointer,
                        });
                        return true;
                    }
                    Drag2d::Point {
                        panel_id,
                        point_idx,
                        start_pointer,
                        mut moved,
                    } => {
                        if (px - start_pointer.0).hypot(py - start_pointer.1) >= 2.5 {
                            moved = true;
                        }
                        if moved
                            && let Some(board_m) = screen_to_board_m(state, px, py)
                            && let Some(panel_idx) = state
                                .project
                                .template
                                .panels
                                .iter()
                                .position(|p| p.id == panel_id)
                        {
                            let local =
                                board_to_panel_pt(&state.project.template, panel_idx, board_m);
                            update_preview_point(state, &panel_id, point_idx, local);
                        }
                        state.drag_2d = Some(Drag2d::Point {
                            panel_id,
                            point_idx,
                            start_pointer,
                            moved,
                        });
                        return true;
                    }
                    Drag2d::BezierHandle {
                        panel_id,
                        edge_name,
                        handle_index,
                        start_pointer,
                        mut moved,
                        mut last_local,
                    } => {
                        if (px - start_pointer.0).hypot(py - start_pointer.1) >= 2.0 {
                            moved = true;
                        }
                        if let Some(board_m) = screen_to_board_m(state, px, py)
                            && let Some(panel_idx) = state
                                .project
                                .template
                                .panels
                                .iter()
                                .position(|p| p.id == panel_id)
                        {
                            last_local =
                                board_to_panel_pt(&state.project.template, panel_idx, board_m);
                            if moved {
                                update_preview_bezier_handle(
                                    state,
                                    &panel_id,
                                    &edge_name,
                                    handle_index,
                                    last_local,
                                );
                            }
                        }
                        state.drag_2d = Some(Drag2d::BezierHandle {
                            panel_id,
                            edge_name,
                            handle_index,
                            start_pointer,
                            moved,
                            last_local,
                        });
                        return true;
                    }
                    Drag2d::CornerRadius {
                        panel_id,
                        edge_name,
                        corner_board_m,
                        bisector,
                        denom,
                        max_radius_m,
                        start_pointer,
                        mut moved,
                    } => {
                        if (px - start_pointer.0).hypot(py - start_pointer.1) >= 2.0 {
                            moved = true;
                        }
                        if moved && let Some(board_m) = screen_to_board_m(state, px, py) {
                            let proj = (board_m - corner_board_m).dot(bisector).max(0.0005);
                            let radius_m = (proj / denom).clamp(0.001, max_radius_m * 0.98);
                            state.corner_radius_mm = radius_m * 1000.0;
                            state.status_message =
                                format!("Corner preview radius: {:.1} mm", state.corner_radius_mm);
                        }
                        state.drag_2d = Some(Drag2d::CornerRadius {
                            panel_id,
                            edge_name,
                            corner_board_m,
                            bisector,
                            denom,
                            max_radius_m,
                            start_pointer,
                            moved,
                        });
                        return true;
                    }
                }
            }

            if state.pointer_down && state.dragging_3d {
                state.camera.yaw += dx * 0.008;
                state.camera.pitch = (state.camera.pitch + dy * 0.008).clamp(-1.45, 1.45);
                return false;
            }

            // Passive hover feedback in 2D viewport
            if !state.pointer_down && !state.command_palette_open && !state.solver_menu_open {
                let new_hover = screen_to_board_m(state, px, py)
                    .and_then(|board_m| hit_test_2d(state, board_m));
                if new_hover != state.hovered_2d {
                    state.hovered_2d = new_hover;
                    return true;
                }
            }
            false
        }
        EngineEvent::Scroll { delta, .. } => {
            if state.command_palette_open || state.solver_menu_open {
                return false;
            }
            let dy = match delta {
                cssimpler::renderer::ScrollDelta::Lines { y, .. } => *y * 0.2,
                cssimpler::renderer::ScrollDelta::Pixels { y, .. } => *y * 0.005,
            };
            let (px, py) = state.last_pointer_pos;
            let (canvas_2d, svg_2d) = compute_2d_viewport_rects(state);
            let rect_3d = viewport_3d_rect(state);
            if canvas_2d.contains(px, py) {
                let (vbx, vby, old_span) = compute_2d_view_box_m(state);
                let u = if svg_2d.w > 0.0 {
                    ((px - svg_2d.x) / svg_2d.w) as f64 - 0.5
                } else {
                    0.0
                };
                let v = if svg_2d.h > 0.0 {
                    ((py - svg_2d.y) / svg_2d.h) as f64 - 0.5
                } else {
                    0.0
                };
                let anchor_x = vbx + (0.5 + u) * old_span;
                let anchor_y = vby + (0.5 + v) * old_span;
                let center_x = vbx + 0.5 * old_span - state.board_2d_pan.0;
                let center_y = vby + 0.5 * old_span - state.board_2d_pan.1;

                let old_zoom = state.board_2d_zoom.clamp(0.25, 10.0);
                let factor = (1.0 + dy as f64 * 0.15).clamp(0.5, 2.0);
                let new_zoom = (old_zoom * factor).clamp(0.25, 10.0);
                let new_span = old_span * (old_zoom / new_zoom);
                state.board_2d_zoom = new_zoom;
                if svg_2d.contains(px, py) {
                    state.board_2d_pan.0 = anchor_x - center_x - u * new_span;
                    state.board_2d_pan.1 = anchor_y - center_y - v * new_span;
                }
                return true;
            } else if rect_3d.contains(px, py) {
                state.camera.distance = (state.camera.distance - dy).clamp(0.5, 15.0);
            }
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
                    <button class="menu-item" onclick_shared={queue_action(&state.actions, ACT_RUN_SAVE)}>"Save Project (Ctrl+S)"</button>
                    <button class="menu-item" onclick_shared={queue_action(&state.actions, ACT_RUN_EXPORT)}>"Export Artifacts"</button>
                </nav>
            </div>
            <div class="transport-controls">
                <button id="sim-play-btn" class={play_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PLAY_PAUSE)} title="Start/Stop simulation (Space)">
                    {if state.playing { icon_pause() } else { icon_play() }}
                    <span class="btn-label">{play_label}</span>
                </button>
                <button id="sim-step-btn" class="transport-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_STEP)} title="Advance single solver step">
                    {icon_step()}
                    <span class="btn-label">"Step"</span>
                </button>
                <button id="sim-reset-btn" class="transport-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_RESET)} title="Reset simulation state">
                    {icon_reset()}
                    <span class="btn-label">"Reset"</span>
                </button>
                <div class="solver-picker">
                    <button id="sim-backend-indicator" class={backend_class} onclick_shared={queue_action(&state.actions, ACT_RUN_SOLVER_MENU)} title="Choose active solver">{format!("{backend_label} ▾")}</button>
                    {if state.solver_menu_open { ui! {
                        <div id="solver-dropdown" class="solver-dropdown">
                            <button id="solver-wgsl-ipc" class="menu-item" onclick_shared={queue_action(&state.actions, ACT_RUN_SOLVER_GPU)}>"WGSL AVBD / IPC"</button>
                            <button id="solver-barrier-al" class="menu-item" onclick_shared={queue_action(&state.actions, ACT_RUN_SOLVER_AL)}>"WGSL AVBD / barrier AL (experimental)"</button>
                            <button id="solver-cpu" class="menu-item" onclick_shared={queue_action(&state.actions, ACT_RUN_SOLVER_CPU)}>"CPU VBD / OGC"</button>
                        </div>
                    }} else { ui! { <span /> } }}
                </div>
            </div>
            <div class="top-bar-right">
                <div class="layout-switcher">
                    <button class={split_class} onclick_shared={queue_action(&state.actions, ACT_RUN_VIEW_SPLIT)} title="Split 3D + 2D View">
                        {icon_view_split()}
                        <span class="btn-label">"Split"</span>
                    </button>
                    <button class={threed_class} onclick_shared={queue_action(&state.actions, ACT_RUN_VIEW_3D)} title="Full 3D Garment View">
                        {icon_view_3d()}
                        <span class="btn-label">"3D"</span>
                    </button>
                    <button class={twod_class} onclick_shared={queue_action(&state.actions, ACT_RUN_VIEW_2D)} title="Full 2D Pattern Board">
                        {icon_view_2d()}
                        <span class="btn-label">"2D"</span>
                    </button>
                </div>
                <button id="clean-view-toggle" class={clean_class} onclick_shared={queue_action(&state.actions, ACT_RUN_CLEAN_VIEW)} title="Minimalist Presentation View">
                    {icon_clean_view()}
                    <span class="btn-label">"Clean View"</span>
                </button>
                <button id="command-palette-trigger" onclick_shared={queue_action(&state.actions, ACT_RUN_PALETTE_TOGGLE)} title="Open Command Palette (Ctrl+K)">
                    {icon_palette()}
                    <span class="btn-label">"Commands"</span>
                </button>
                <button id="theme-toggle" onclick_shared={queue_action(&state.actions, ACT_RUN_THEME_TOGGLE)} title={theme_label}>
                    {icon_day_night_switch(matches!(state.theme, Theme::Dark))}
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

    ui! {
        <aside id="rail-3d" class="tool-rail tool-rail-3d">
            <button class={gizmo_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_GIZMO)} title="Select & Arrange [Gizmo] (1)">
                {icon_gizmo()}
                <span class="tool-label">"Gizmo"</span>
            </button>
            <button class={grab_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_GRAB)} title="Grab Cloth (Pull & Drag in Simulation) (W)">
                {icon_grab()}
                <span class="tool-label">"Grab"</span>
            </button>
            <button class={sew3d_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_SEW3D)} title="3D Segment Sewing (N)">
                {icon_sew_3d()}
                <span class="tool-label">"3D Sew"</span>
            </button>
            <button class={pin_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_PIN)} title="Pin Cloth [Click vertices] (P)">
                {icon_pin()}
                <span class="tool-label">"Pin"</span>
            </button>
            <button class={msr_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_MEASURE3D)} title="3D Tape Measure (T)">
                {icon_measure_3d()}
                <span class="tool-label">"Measure"</span>
            </button>
            <button id="avatar-diagnostics-btn" class={avatar_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_AVATAR_DIAG)} title="Avatar & Dress Form Diagnostics (Shift+A)">
                {icon_dress_form()}
                <span class="tool-label">"Avatar"</span>
            </button>
            <button class="tool-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_CAM_RESET)} title="Reset Camera View [Framing] (0)">
                {icon_cam_reset()}
                <span class="tool-label">"Reset"</span>
            </button>
            <button class="tool-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_DRAWER_TOGGLE)} title="Toggle Asset & Piece Drawer (Tab)">
                {icon_sidebar_toggle()}
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

    ui! {
        <aside id="rail-2d" class="tool-rail tool-rail-2d">
            <button class={edit_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_EDIT2D)} title="Edit Point / Transform (Z / A)">
                {icon_edit_2d()}
                <span class="tool-label">"Edit"</span>
            </button>
            <button class={rect_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_RECT2D)} title="Draw Rectangle Pattern (S)">
                {icon_rect_2d()}
                <span class="tool-label">"Rect"</span>
            </button>
            <button class={pen_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_PEN2D)} title="Draw Polygon Pattern (H)">
                {icon_polygon_2d()}
                <span class="tool-label">"Pen"</span>
            </button>
            <button class={free_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_FREE2D)} title="Freehand Pattern Curve (F)">
                {icon_freehand_2d()}
                <span class="tool-label">"Free"</span>
            </button>
            <button class={sew_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_SEW2D)} title="Sew Seams (B)">
                {icon_sew_2d()}
                <span class="tool-label">"Sew"</span>
            </button>
            <button class={split_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_SPLIT2D)} title="Split / Add Point (X)">
                {icon_split_2d()}
                <span class="tool-label">"Split"</span>
            </button>
            <button class={line_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_INTERNAL2D)} title="Internal Polygon / Line (G)">
                {icon_internal_2d()}
                <span class="tool-label">"Line"</span>
            </button>
            <button class={tape_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_MEASURE2D)} title="2D Tape Measure (M)">
                {icon_measure_2d()}
                <span class="tool-label">"Tape"</span>
            </button>
            <button class={trim_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_TRIM2D)} title="Trim (T)">
                {icon_trim_2d()}
                <span class="tool-label">"Trim"</span>
            </button>
            <button class={corner_class} onclick_shared={queue_action(&state.actions, ACT_RUN_TOOL_CORNER2D)} title="Corner Rounding & Chamfer (C)">
                {icon_corner_2d()}
                <span class="tool-label">"Corner"</span>
            </button>
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
        let card_node = match idx {
            0 => ui! {
                <div class={card_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_0)}>
                    <div class="piece-header">
                        <span class="piece-name">{label}</span>
                        <span class="piece-id">{id_str}</span>
                    </div>
                    <div class="piece-meta">
                        <span>{meta_str}</span>
                    </div>
                </div>
            },
            1 => ui! {
                <div class={card_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_1)}>
                    <div class="piece-header">
                        <span class="piece-name">{label}</span>
                        <span class="piece-id">{id_str}</span>
                    </div>
                    <div class="piece-meta">
                        <span>{meta_str}</span>
                    </div>
                </div>
            },
            2 => ui! {
                <div class={card_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_2)}>
                    <div class="piece-header">
                        <span class="piece-name">{label}</span>
                        <span class="piece-id">{id_str}</span>
                    </div>
                    <div class="piece-meta">
                        <span>{meta_str}</span>
                    </div>
                </div>
            },
            _ => ui! {
                <div class={card_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_3)}>
                    <div class="piece-header">
                        <span class="piece-name">{label}</span>
                        <span class="piece-id">{id_str}</span>
                    </div>
                    <div class="piece-meta">
                        <span>{meta_str}</span>
                    </div>
                </div>
            },
        };
        piece_cards.push(card_node);
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
                            <button class="btn-action" onclick_shared={queue_action(&state.actions, ACT_RUN_EXPORT)}>"Export OBJ"</button>
                            <button class="btn-action btn-action-accent" onclick_shared={queue_action(&state.actions, ACT_RUN_EXPORT)}>"True-Scale SVG"</button>
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
                    <button class="action-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_NEXT)}>"Select Next Piece"</button>
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

pub fn build_runner_2d_viewport(state: &CssRunnerState) -> Node {
    let (svg_nodes, view_box) = render_runner_2d_svg(state);
    let (_canvas_rect, svg_rect) = compute_2d_viewport_rects(state);
    let svg_w = format!("{:.1}px", svg_rect.w);
    let svg_h = format!("{:.1}px", svg_rect.h);

    let panels = &state.project.template.panels;
    let selected_panel = panels.get(state.selected_piece_index);
    let selected_name = selected_panel.map_or("None", |p| p.name.as_str());
    let (w_mm, h_mm) = selected_panel.map_or((0.0, 0.0), |p| {
        let (w, h) = if let Some(poly) = &p.polygon {
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
            (max_x - min_x, max_y - min_y)
        } else {
            (p.pattern.width, p.pattern.height)
        };
        (w * 1000.0, h * 1000.0)
    });

    let zoom_pct = (state.board_2d_zoom * 100.0).round() as i32;
    let sub_detail = if let Some((ref pid, pt_idx)) = state.selected_2d_point {
        let edge_label = state
            .project
            .template
            .panels
            .iter()
            .find(|p| &p.id == pid)
            .and_then(|p| p.polygon.as_ref())
            .and_then(|poly| poly.edges.get(pt_idx))
            .map_or("?", |e| e.as_str());
        format!(" · Corner #{pt_idx} ({edge_label})")
    } else if let Some((ref _pid, ref ename)) = state.selected_2d_edge {
        format!(" · Edge {ename}")
    } else {
        String::new()
    };
    let sub_text = format!(
        "Tool: {} · Zoom: {}% · Selected: {} ({:.0}×{:.0} mm){}",
        state.active_2d_tool, zoom_pct, selected_name, w_mm, h_mm, sub_detail
    );

    let radius_mode_class = if state.corner_mode_radius {
        "cad-2d-pill active"
    } else {
        "cad-2d-pill"
    };
    let shape_mode_class = if !state.corner_mode_radius {
        "cad-2d-pill active"
    } else {
        "cad-2d-pill"
    };
    let sharp_class = if state.corner_rule == CornerRule::Sharp {
        "cad-2d-pill active"
    } else {
        "cad-2d-pill"
    };
    let smooth_class = if state.corner_rule == CornerRule::Smooth {
        "cad-2d-pill active"
    } else {
        "cad-2d-pill"
    };
    let sym_class = if state.corner_rule == CornerRule::Symmetric {
        "cad-2d-pill active"
    } else {
        "cad-2d-pill"
    };
    let undo_label = if state.history.can_undo() {
        "Undo"
    } else {
        "Undo ·"
    };
    let redo_label = if state.history.can_redo() {
        "Redo"
    } else {
        "Redo ·"
    };

    ui! {
        <div id="viewport-2d" class="viewport-container">
            <div class="viewport-header">
                <div class="viewport-header-left">
                    <span class="viewport-title">"2D Pattern Board"</span>
                    <span class="viewport-sub">{sub_text}</span>
                </div>
                <div class="viewport-tools">
                    <button class="viewport-tool-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_2D_ZOOM_IN)} title="Zoom In (+)">"+"</button>
                    <button class="viewport-tool-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_2D_ZOOM_OUT)} title="Zoom Out (-)">"-"</button>
                    <button class="viewport-tool-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_2D_FIT)} title="Fit Board (0)">"Fit"</button>
                    <button class="viewport-tool-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_2D_NEXT_PIECE)} title="Cycle Piece (])">"Next Piece ▾"</button>
                </div>
            </div>
            <div id="cad-2d-bar" class="cad-2d-bar">
                <div class="cad-2d-group">
                    <button id="cad-undo-btn" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::Undo2d)} title="Undo (Ctrl+Z)">{undo_label}</button>
                    <button id="cad-redo-btn" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::Redo2d)} title="Redo (Ctrl+Shift+Z)">{redo_label}</button>
                </div>
                <div class="cad-2d-group">
                    <span class="cad-2d-label">"Corner"</span>
                    <button id="cad-corner-radius-mode" class={radius_mode_class} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerMode(true))} title="Circular fillet mode">"Radius"</button>
                    <button id="cad-corner-shape-mode" class={shape_mode_class} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerMode(false))} title="Shaped tension fillet mode">"Shape"</button>
                    <button id="cad-radius-dec" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::AdjustCornerRadiusMm(-2.0))} title="Decrease corner radius">"-2"</button>
                    <span id="cad-radius-readout" class="cad-2d-value">{format!("{:.1} mm", state.corner_radius_mm)}</span>
                    <button id="cad-radius-inc" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::AdjustCornerRadiusMm(2.0))} title="Increase corner radius">"+2"</button>
                    <button id="cad-tension-dec" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::AdjustCornerTension(-0.05))} title="Decrease tension">"T-"</button>
                    <span id="cad-tension-readout" class="cad-2d-value">{format!("{:.2}", state.corner_tension)}</span>
                    <button id="cad-tension-inc" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::AdjustCornerTension(0.05))} title="Increase tension">"T+"</button>
                    <button id="cad-apply-round-btn" class="cad-2d-pill primary" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::ApplyCornerRoundSelected)} title="Round selected corner (Enter)">"Round"</button>
                </div>
                <div class="cad-2d-group">
                    <span class="cad-2d-label">"Rule"</span>
                    <button id="cad-rule-sharp" class={sharp_class} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerRule(CornerRule::Sharp))} title="Sharp independent handles">"Sharp"</button>
                    <button id="cad-rule-smooth" class={smooth_class} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerRule(CornerRule::Smooth))} title="Smooth G1 collinear handles">"Smooth"</button>
                    <button id="cad-rule-sym" class={sym_class} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerRule(CornerRule::Symmetric))} title="Symmetric equal-length handles">"Sym"</button>
                    <button id="cad-curve-on-btn" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::ToggleSelectedCurve(true))} title="Convert selected boundary/corner to cubic Bézier">"Curve"</button>
                    <button id="cad-curve-off-btn" class="cad-2d-pill" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::ToggleSelectedCurve(false))} title="Straighten selected boundary/corner">"Straight"</button>
                </div>
            </div>
            <div class="canvas-area">
                <svg id="svg-2d-board" viewBox={view_box} preserveAspectRatio="xMidYMid meet" width={svg_w} height={svg_h} class="svg-2d-board">
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
                    <button class={tp} onclick_shared={queue_action(&state.actions, ACT_RUN_TAB_PATTERN)}>"2D Pattern"</button>
                    <button class={ts} onclick_shared={queue_action(&state.actions, ACT_RUN_TAB_SEWING)}>"Sewing"</button>
                    <button class={tf} onclick_shared={queue_action(&state.actions, ACT_RUN_TAB_FORM)}>"3D Form"</button>
                    <button class={tm} onclick_shared={queue_action(&state.actions, ACT_RUN_TAB_MATERIAL)}>"Material"</button>
                    <button class={te} onclick_shared={queue_action(&state.actions, ACT_RUN_TAB_ENV)}>"Environment"</button>
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
        let grain_str = format!("{:.0}°", p.grain_degrees);
        let row_node = match idx {
            0 => ui! {
                <div class={item_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_0)} style="cursor:pointer">
                    <span>{label}</span>
                    <span>{grain_str}</span>
                </div>
            },
            1 => ui! {
                <div class={item_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_1)} style="cursor:pointer">
                    <span>{label}</span>
                    <span>{grain_str}</span>
                </div>
            },
            2 => ui! {
                <div class={item_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_2)} style="cursor:pointer">
                    <span>{label}</span>
                    <span>{grain_str}</span>
                </div>
            },
            _ => ui! {
                <div class={item_class} onclick_shared={queue_action(&state.actions, ACT_RUN_PIECE_3)} style="cursor:pointer">
                    <span>{label}</span>
                    <span>{grain_str}</span>
                </div>
            },
        };
        piece_rows.push(row_node);
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

    let open_0 = state.accordion_open[0];
    let open_1 = state.accordion_open[1];
    let open_2 = state.accordion_open[2];
    let open_3 = state.accordion_open[3];
    let open_4 = state.accordion_open[4];
    let open_5 = state.accordion_open[5];

    // Accordion 0: Information
    let acc_info = ui! {
        <div class={if open_0 { "accordion open" } else { "accordion collapsed" }}>
            <button class="accordion-header" onclick_shared={queue_action(&state.actions, ACT_RUN_ACC_0)}>
                <span>"1. Piece Information"</span>
                {if open_0 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            {if open_0 {
                ui! {
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
                }
            } else {
                ui! { <span class="hidden" /> }
            }}
        </div>
    };

    // Accordion 1: Selected Line, Corner & Seam Allowance
    let template = state.active_template();
    let sel_info: (String, String, String, String) = if let Some((ref pid, pt_idx)) =
        state.selected_2d_point
    {
        if let Some(p) = template.panels.iter().find(|p| &p.id == pid)
            && let Some(ref poly) = p.polygon
            && let Some(edge_name) = poly.edges.get(pt_idx)
        {
            let prev_name = &poly.edges[(pt_idx + poly.edges.len() - 1) % poly.edges.len()];
            let curved = poly.curves.contains_key(edge_name) || poly.curves.contains_key(prev_name);
            let ctype = if curved { "Cubic Bézier" } else { "Straight" }.to_string();
            let len_str = panel_edge_length_m(p, edge_name)
                .map_or("—".into(), |m| format!("{:.1} mm", m * 1000.0));
            let sa_str = format!("{:.1} mm", poly.allowance_of(edge_name) * 1000.0);
            (
                format!("Corner #{pt_idx} ({edge_name})"),
                ctype,
                len_str,
                sa_str,
            )
        } else {
            (
                format!("Corner #{pt_idx}"),
                "—".into(),
                "—".into(),
                "0.0 mm".into(),
            )
        }
    } else if let Some((ref pid, ref ename)) = state.selected_2d_edge {
        if let Some(p) = template.panels.iter().find(|p| &p.id == pid)
            && let Some(ref poly) = p.polygon
        {
            let ctype = if poly.curves.contains_key(ename) {
                "Cubic Bézier"
            } else {
                "Straight"
            }
            .to_string();
            let len_str = panel_edge_length_m(p, ename)
                .map_or("—".into(), |m| format!("{:.1} mm", m * 1000.0));
            let sa_str = format!("{:.1} mm", poly.allowance_of(ename) * 1000.0);
            (format!("Edge {ename}"), ctype, len_str, sa_str)
        } else {
            (
                format!("Edge {ename}"),
                "—".into(),
                "—".into(),
                "0.0 mm".into(),
            )
        }
    } else {
        let curve_count = selected_panel
            .and_then(|p| p.polygon.as_ref())
            .map_or(0, |poly| poly.curves.len());
        (
            "Click point or edge".into(),
            if curve_count > 0 {
                format!("{curve_count} Bézier curves")
            } else {
                "Straight contour".into()
            },
            "—".into(),
            "0.0 mm".into(),
        )
    };
    let rule_sharp_btn = if state.corner_rule == CornerRule::Sharp {
        "cad-prop-btn active"
    } else {
        "cad-prop-btn"
    };
    let rule_smooth_btn = if state.corner_rule == CornerRule::Smooth {
        "cad-prop-btn active"
    } else {
        "cad-prop-btn"
    };
    let rule_sym_btn = if state.corner_rule == CornerRule::Symmetric {
        "cad-prop-btn active"
    } else {
        "cad-prop-btn"
    };

    let acc_line = ui! {
        <div class={if open_1 { "accordion open" } else { "accordion collapsed" }}>
            <button class="accordion-header" onclick_shared={queue_action(&state.actions, ACT_RUN_ACC_1)}>
                <span>"2. Boundary & Allowance"</span>
                {if open_1 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            {if open_1 {
                ui! {
                    <div class="accordion-content">
                        <div class="prop-row">
                            <span class="prop-label">"2D Selection"</span>
                            <span class="prop-value">{sel_info.0}</span>
                        </div>
                        <div class="prop-row">
                            <span class="prop-label">"Curvature Type"</span>
                            <span class="prop-value">{sel_info.1}</span>
                        </div>
                        <div class="prop-row">
                            <span class="prop-label">"Boundary Length"</span>
                            <span class="prop-value">{sel_info.2}</span>
                        </div>
                        <div class="prop-row">
                            <span class="prop-label">"Seam Allowance"</span>
                            <span class="prop-value">{sel_info.3}</span>
                        </div>
                        <div class="prop-row">
                            <span class="prop-label">"Fillet Radius"</span>
                            <span class="prop-value">{format!("{:.1} mm (T {:.2})", state.corner_radius_mm, state.corner_tension)}</span>
                        </div>
                        <div class="cad-prop-actions">
                            <button class={rule_sharp_btn} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerRule(CornerRule::Sharp))}>"Sharp"</button>
                            <button class={rule_smooth_btn} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerRule(CornerRule::Smooth))}>"Smooth"</button>
                            <button class={rule_sym_btn} onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::SetCornerRule(CornerRule::Symmetric))}>"Sym"</button>
                        </div>
                        <div class="cad-prop-actions">
                            <button class="cad-prop-btn primary" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::ApplyCornerRoundSelected)}>{format!("Round ({:.0} mm)", state.corner_radius_mm)}</button>
                            <button class="cad-prop-btn" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::ToggleSelectedCurve(true))}>"To Bézier"</button>
                            <button class="cad-prop-btn" onclick_shared={queue_ui_op(&state.pending_ui_ops, RunnerUiOp::ToggleSelectedCurve(false))}>"To Straight"</button>
                        </div>
                    </div>
                }
            } else {
                ui! { <span class="hidden" /> }
            }}
        </div>
    };

    // Accordion 2: 3D Arrangement
    let acc_arrange = ui! {
        <div class={if open_2 { "accordion open" } else { "accordion collapsed" }}>
            <button class="accordion-header" onclick_shared={queue_action(&state.actions, ACT_RUN_ACC_2)}>
                <span>"3. 3D Arrangement"</span>
                {if open_2 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            {if open_2 {
                ui! {
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
                }
            } else {
                ui! { <span class="hidden" /> }
            }}
        </div>
    };

    // Accordion 3: Pins
    let acc_pins = ui! {
        <div class={if open_3 { "accordion open" } else { "accordion collapsed" }}>
            <button class="accordion-header" onclick_shared={queue_action(&state.actions, ACT_RUN_ACC_3)}>
                <span>"4. Pins & Restraints"</span>
                {if open_3 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            {if open_3 {
                ui! {
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
                }
            } else {
                ui! { <span class="hidden" /> }
            }}
        </div>
    };

    // Accordion 4: Fabric & Mechanical Properties
    let acc_fabric = ui! {
        <div class={if open_4 { "accordion open" } else { "accordion collapsed" }}>
            <button class="accordion-header" onclick_shared={queue_action(&state.actions, ACT_RUN_ACC_4)}>
                <span>"5. Fabric Mechanics"</span>
                {if open_4 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>
            {if open_4 {
                ui! {
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
                }
            } else {
                ui! { <span class="hidden" /> }
            }}
        </div>
    };

    // Accordion 5: Simulation Diagnostics
    let acc_sim = ui! {
        <div class={if open_5 { "accordion open" } else { "accordion collapsed" }}>
            <button class="accordion-header" onclick_shared={queue_action(&state.actions, ACT_RUN_ACC_5)}>
                <span>"6. Simulation Diagnostics"</span>
                {if open_5 { icon_chevron_down() } else { icon_chevron_right() }}
            </button>

            {if open_5 {
                ui! {
                    <div class="accordion-content">
                        <div class="prop-row">
                            <span class="prop-label">"Solver Step Time"</span>
                            <span class="prop-value">{format!("{:.2} ms", state.step_ms)}</span>
                        </div>
                        <div class="prop-row">
                            <span class="prop-label">"Force Residual"</span>
                            <span class="prop-value">{last_res}</span>
                        </div>
                        <div class="prop-row">
                            <span class="prop-label">"Minimum Detected Gap"</span>
                            <span class="prop-value">{last_pen}</span>
                        </div>
                        <div class="prop-row">
                            <span class="prop-label">"Active Backend"</span>
                            <span class="prop-value">{if state.gpu_sim { state.contact_method.label() } else { "CPU (OGC)" }}</span>
                        </div>
                    </div>
                }
            } else {
                ui! { <span class="hidden" /> }
            }}

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
                    <button class="palette-close-btn" onclick_shared={queue_action(&state.actions, ACT_RUN_PALETTE_TOGGLE)}>"✕"</button>
                </div>
                <div class="palette-input">
                    "Type a command or shortcut..."
                </div>
                <div class="palette-items">
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_PLAY_PAUSE)}>
                        <div class="item-left">
                            {if state.playing { icon_pause() } else { icon_play() }}
                            <span>"Toggle Simulation"</span>
                        </div>
                        <span class="command-shortcut">"Space"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_STEP)}>
                        <div class="item-left">
                            {icon_step()}
                            <span>"Step Simulation"</span>
                        </div>
                        <span class="command-shortcut">"S"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_RESET)}>
                        <div class="item-left">
                            {icon_reset()}
                            <span>"Reset Simulation"</span>
                        </div>
                        <span class="command-shortcut">"R"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_SAVE)}>
                        <div class="item-left">
                            {icon_save()}
                            <span>"Save Fitting Project"</span>
                        </div>
                        <span class="command-shortcut">"Ctrl+S"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_EXPORT)}>
                        <div class="item-left">
                            {icon_export()}
                            <span>"Export Artifacts (OBJ & True-Scale SVG)"</span>
                        </div>
                        <span class="command-shortcut">"Ctrl+E"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_CLEAN_VIEW)}>
                        <div class="item-left">
                            {icon_clean_view()}
                            <span>"Toggle Clean View"</span>
                        </div>
                        <span class="command-shortcut">"F11"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_THEME_TOGGLE)}>
                        <div class="item-left">
                            {if matches!(state.theme, Theme::Dark) { icon_sun() } else { icon_moon() }}
                            <span>"Toggle Dark / Light Theme"</span>
                        </div>
                        <span class="command-shortcut">"T"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_VIEW_SPLIT)}>
                        <div class="item-left">
                            {icon_view_split()}
                            <span>"Split 2D / 3D Layout"</span>
                        </div>
                        <span class="command-shortcut">"1"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_VIEW_3D)}>
                        <div class="item-left">
                            {icon_view_3d()}
                            <span>"3D Only Viewport"</span>
                        </div>
                        <span class="command-shortcut">"2"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_VIEW_2D)}>
                        <div class="item-left">
                            {icon_view_2d()}
                            <span>"2D Only Pattern Board"</span>
                        </div>
                        <span class="command-shortcut">"3"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_CAM_RESET)}>
                        <div class="item-left">
                            {icon_cam_reset()}
                            <span>"Reset 3D Camera"</span>
                        </div>
                        <span class="command-shortcut">"Home"</span>
                    </button>
                    <button class="palette-item" onclick_shared={queue_action(&state.actions, ACT_RUN_DRAWER_TOGGLE)}>
                        <div class="item-left">
                            {icon_sidebar_toggle()}
                            <span>"Toggle Left Asset Drawer"</span>
                        </div>
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

pub fn render_runner_2d_svg(state: &CssRunnerState) -> (Vec<Node>, String) {
    let mut nodes = Vec::new();
    let template = state.active_template();
    let panels = &template.panels;

    struct PanelCurve {
        edge_idx: usize,
        edge_name: String,
        c1: (f64, f64),
        c2: (f64, f64),
    }

    struct PanelGeo {
        panel_idx: usize,
        id: String,
        points: Vec<(f64, f64)>,
        edge_names: Vec<String>,
        curves: Vec<PanelCurve>,
        internal_lines: Vec<Vec<(f64, f64)>>,
        holes: Vec<Vec<(f64, f64)>>,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        center: (f64, f64),
        grain_deg: f64,
        is_selected: bool,
    }

    let to_mm = |p: DVec2| -> (f64, f64) { (p.x * 1000.0, p.y * 1000.0) };

    let mut panel_geos = Vec::with_capacity(panels.len());
    for (idx, panel) in panels.iter().enumerate() {
        let is_selected = idx == state.selected_piece_index;
        let raw_pts = panel_raw_points(panel);
        let points: Vec<(f64, f64)> = raw_pts
            .iter()
            .map(|&pt| to_mm(panel_to_board_pt(template, idx, pt)))
            .collect();

        let mut edge_names = Vec::new();
        let mut curves = Vec::new();
        let mut internal_lines = Vec::new();
        let mut holes = Vec::new();

        if let Some(ref poly) = panel.polygon {
            edge_names = poly.edges.clone();
            for (edge_idx, edge_name) in poly.edges.iter().enumerate() {
                if let Some([c1, c2]) = poly.curves.get(edge_name) {
                    curves.push(PanelCurve {
                        edge_idx,
                        edge_name: edge_name.clone(),
                        c1: to_mm(panel_to_board_pt(template, idx, *c1)),
                        c2: to_mm(panel_to_board_pt(template, idx, *c2)),
                    });
                }
            }
            for line in &poly.lines {
                let pts: Vec<(f64, f64)> = line
                    .points
                    .iter()
                    .map(|&p| to_mm(panel_to_board_pt(template, idx, p)))
                    .collect();
                if pts.len() >= 2 {
                    internal_lines.push(pts);
                }
            }
            for hole in &poly.holes {
                let pts: Vec<(f64, f64)> = hole
                    .points
                    .iter()
                    .map(|&p| to_mm(panel_to_board_pt(template, idx, p)))
                    .collect();
                if pts.len() >= 3 {
                    holes.push(pts);
                }
            }
        } else {
            for i in 0..points.len() {
                edge_names.push(format!("edge_{i}"));
            }
        }

        let mut s_min_x = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let mut s_max_x = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        let mut s_min_y = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let mut s_max_y = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        for c in &curves {
            s_min_x = s_min_x.min(c.c1.0).min(c.c2.0);
            s_max_x = s_max_x.max(c.c1.0).max(c.c2.0);
            s_min_y = s_min_y.min(c.c1.1).min(c.c2.1);
            s_max_y = s_max_y.max(c.c1.1).max(c.c2.1);
        }
        let center = ((s_min_x + s_max_x) * 0.5, (s_min_y + s_max_y) * 0.5);

        panel_geos.push(PanelGeo {
            panel_idx: idx,
            id: panel.id.clone(),
            points,
            edge_names,
            curves,
            internal_lines,
            holes,
            min_x: s_min_x,
            min_y: s_min_y,
            max_x: s_max_x,
            max_y: s_max_y,
            center,
            grain_deg: panel.grain_degrees,
            is_selected,
        });
    }

    // Millimetre SVG user space gives cssimpler flatten_cubic ~0.25 mm accuracy and crisp geometry
    let (vb_min_x_m, vb_min_y_m, view_span_m) = compute_2d_view_box_m(state);
    let vb_min_x = vb_min_x_m * 1000.0;
    let vb_min_y = vb_min_y_m * 1000.0;
    let vb_w = view_span_m * 1000.0;
    let vb_h = vb_w;
    let view_box = format!("{vb_min_x:.3} {vb_min_y:.3} {vb_w:.3} {vb_h:.3}");

    let (_canvas_rect, svg_rect) = compute_2d_viewport_rects(state);
    let px = (vb_w / (svg_rect.w.max(1.0) as f64)).max(0.25);

    // Theme colors (use rgba directly since cssimpler ignores stroke-opacity)
    let is_dark = state.theme == Theme::Dark;
    let bg_color = if is_dark { "#0d0f17" } else { "#f8fafc" };
    let grid_minor = if is_dark { "#161c28" } else { "#e2e8f0" };
    let grid_major = if is_dark { "#232e42" } else { "#cbd5e1" };
    let origin_color = if is_dark {
        "rgba(56, 189, 248, 0.28)"
    } else {
        "rgba(37, 99, 235, 0.24)"
    };
    let unsel_stroke = if is_dark { "#38bdf8" } else { "#2563eb" };
    let unsel_fill = if is_dark {
        "rgba(56, 189, 248, 0.08)"
    } else {
        "rgba(37, 99, 235, 0.06)"
    };
    let sel_stroke = if is_dark { "#f59e0b" } else { "#d97706" };
    let sel_fill = if is_dark {
        "rgba(245, 158, 11, 0.14)"
    } else {
        "rgba(217, 119, 6, 0.11)"
    };
    let handle_color = if is_dark { "#38bdf8" } else { "#0284c7" };
    let handle_soft = if is_dark {
        "rgba(56, 189, 248, 0.45)"
    } else {
        "rgba(2, 132, 199, 0.40)"
    };
    let vertex_fill = if is_dark { "#0f172a" } else { "#ffffff" };
    let grain_color = if is_dark {
        "rgba(96, 165, 250, 0.72)"
    } else {
        "rgba(59, 130, 246, 0.70)"
    };
    let accent_gold = "#facc15";
    let accent_emerald = "#10b981";
    let hover_cyan = "#7dd3fc";

    // Background rect
    nodes.push(ui! {
        <rect x={format!("{vb_min_x:.2}")} y={format!("{vb_min_y:.2}")} width={format!("{vb_w:.2}")} height={format!("{vb_h:.2}")} fill={bg_color} />
    });

    // Drafting grid (minor 100 mm, major 500 mm)
    let grid_step = 100.0_f64;
    let start_gx = (vb_min_x / grid_step).floor() * grid_step;
    let mut gx = start_gx;
    while gx <= vb_min_x + vb_w {
        let is_major = ((gx / 500.0) - (gx / 500.0).round()).abs() < 1e-4;
        let stroke = if is_major { grid_major } else { grid_minor };
        let sw = if is_major { 1.1 * px } else { 0.7 * px };
        let pts = format!("{gx:.1},{:.1} {gx:.1},{:.1}", vb_min_y, vb_min_y + vb_h);
        nodes.push(ui! {
            <polyline points={pts} fill="none" stroke={stroke} stroke-width={format!("{sw:.2}")} />
        });
        gx += grid_step;
    }
    let start_gy = (vb_min_y / grid_step).floor() * grid_step;
    let mut gy = start_gy;
    while gy <= vb_min_y + vb_h {
        let is_major = ((gy / 500.0) - (gy / 500.0).round()).abs() < 1e-4;
        let stroke = if is_major { grid_major } else { grid_minor };
        let sw = if is_major { 1.1 * px } else { 0.7 * px };
        let pts = format!("{:.1},{gy:.1} {:.1},{gy:.1}", vb_min_x, vb_min_x + vb_w);
        nodes.push(ui! {
            <polyline points={pts} fill="none" stroke={stroke} stroke-width={format!("{sw:.2}")} />
        });
        gy += grid_step;
    }

    // Origin axes reference
    if vb_min_x <= 0.0 && vb_min_x + vb_w >= 0.0 {
        nodes.push(ui! {
            <polyline points={format!("0.0,{:.1} 0.0,{:.1}", vb_min_y, vb_min_y + vb_h)} fill="none" stroke={origin_color} stroke-width={format!("{:.2}", 1.2 * px)} />
        });
    }
    if vb_min_y <= 0.0 && vb_min_y + vb_h >= 0.0 {
        nodes.push(ui! {
            <polyline points={format!("{:.1},0.0 {:.1},0.0", vb_min_x, vb_min_x + vb_w)} fill="none" stroke={origin_color} stroke-width={format!("{:.2}", 1.2 * px)} />
        });
    }

    let fillet_preview = active_fillet_preview(state);
    let corner_tool_active = state.active_2d_tool == "Corner";

    // Panels rendering
    for p in &panel_geos {
        let is_hovered_panel =
            matches!(state.hovered_2d, Some(Hover2dTarget::Panel(i)) if i == p.panel_idx);
        let stroke_color = if p.is_selected {
            sel_stroke
        } else if is_hovered_panel {
            hover_cyan
        } else {
            unsel_stroke
        };
        let fill_color = if p.is_selected { sel_fill } else { unsel_fill };
        let stroke_w = if p.is_selected { 2.0 * px } else { 1.3 * px };

        // Outer boundary path (with cubic Bézier curves)
        let n_pts = p.points.len();
        if n_pts > 0 {
            let mut path_d = format!("M {:.2},{:.2}", p.points[0].0, p.points[0].1);
            for i in 0..n_pts {
                let next_idx = (i + 1) % n_pts;
                let (next_x, next_y) = p.points[next_idx];
                if let Some(c) = p.curves.iter().find(|c| c.edge_idx == i) {
                    path_d.push_str(&format!(
                        " C {:.2},{:.2} {:.2},{:.2} {next_x:.2},{next_y:.2}",
                        c.c1.0, c.c1.1, c.c2.0, c.c2.1
                    ));
                } else {
                    path_d.push_str(&format!(" L {next_x:.2},{next_y:.2}"));
                }
            }
            path_d.push_str(" Z");

            nodes.push(ui! {
                <path d={path_d} fill={fill_color} stroke={stroke_color} stroke-width={format!("{stroke_w:.2}")} class="pattern-piece" />
            });
        }

        // Hovered or selected edge overlay
        for (edge_idx, edge_name) in p.edge_names.iter().enumerate() {
            let is_sel_edge = state
                .selected_2d_edge
                .as_ref()
                .is_some_and(|(pid, en)| pid == &p.id && en == edge_name);
            let is_hov_edge = matches!(
                state.hovered_2d,
                Some(Hover2dTarget::Edge { panel_idx, edge_idx: ei }) if panel_idx == p.panel_idx && ei == edge_idx
            );
            if is_sel_edge || is_hov_edge {
                let (a_x, a_y) = p.points[edge_idx];
                let (b_x, b_y) = p.points[(edge_idx + 1) % n_pts];
                let seg_d = if let Some(c) = p.curves.iter().find(|c| c.edge_idx == edge_idx) {
                    format!(
                        "M {a_x:.2},{a_y:.2} C {:.2},{:.2} {:.2},{:.2} {b_x:.2},{b_y:.2}",
                        c.c1.0, c.c1.1, c.c2.0, c.c2.1
                    )
                } else {
                    format!("M {a_x:.2},{a_y:.2} L {b_x:.2},{b_y:.2}")
                };
                let e_col = if is_sel_edge { accent_gold } else { hover_cyan };
                let e_w = if is_sel_edge { 3.0 * px } else { 2.4 * px };
                nodes.push(ui! {
                    <path d={seg_d} fill="none" stroke={e_col} stroke-width={format!("{e_w:.2}")} />
                });
            }
        }

        // Internal lines (dashed M..L path)
        let il_col = if is_dark { "#64748b" } else { "#94a3b8" };
        for line_pts in &p.internal_lines {
            let d = dashed_path_d(line_pts, 5.0 * px, 4.0 * px);
            if !d.is_empty() {
                nodes.push(ui! {
                    <path d={d} fill="none" stroke={il_col} stroke-width={format!("{:.2}", 1.1 * px)} />
                });
            }
        }

        // Holes
        for hole_pts in &p.holes {
            let h_pts_str: String = hole_pts
                .iter()
                .map(|(x, y)| format!("{x:.2},{y:.2}"))
                .collect::<Vec<_>>()
                .join(" ");
            nodes.push(ui! {
                <polygon points={h_pts_str} fill={bg_color} stroke={stroke_color} stroke-width={format!("{:.2}", 1.1 * px)} />
            });
        }

        // Bézier Indicators, Tangent Stalks & Handles
        let panel_ref = &panels[p.panel_idx];
        let show_full_handles = p.is_selected
            || matches!(
                state.hovered_2d,
                Some(Hover2dTarget::BezierHandle { panel_idx, .. }) if panel_idx == p.panel_idx
            );
        for c in &p.curves {
            let (p0_x, p0_y) = p.points[c.edge_idx];
            let (p3_x, p3_y) = p.points[(c.edge_idx + 1) % n_pts];
            let stalk_col = if show_full_handles {
                handle_color
            } else {
                handle_soft
            };
            let d1 = dashed_path_d(&[(p0_x, p0_y), c.c1], 3.5 * px, 2.5 * px);
            let d2 = dashed_path_d(&[(p3_x, p3_y), c.c2], 3.5 * px, 2.5 * px);
            nodes.push(ui! {
                <path d={d1} fill="none" stroke={stalk_col} stroke-width={format!("{:.2}", 1.0 * px)} />
            });
            nodes.push(ui! {
                <path d={d2} fill="none" stroke={stalk_col} stroke-width={format!("{:.2}", 1.0 * px)} />
            });

            let h0_active = matches!(
                state.hovered_2d,
                Some(Hover2dTarget::BezierHandle { panel_idx, edge_idx, handle_index: 0 })
                    if panel_idx == p.panel_idx && edge_idx == c.edge_idx
            );
            let h1_active = matches!(
                state.hovered_2d,
                Some(Hover2dTarget::BezierHandle { panel_idx, edge_idx, handle_index: 1 })
                    if panel_idx == p.panel_idx && edge_idx == c.edge_idx
            );
            let hr0 = if h0_active {
                4.8 * px
            } else if show_full_handles {
                3.6 * px
            } else {
                2.5 * px
            };
            let hr1 = if h1_active {
                4.8 * px
            } else if show_full_handles {
                3.6 * px
            } else {
                2.5 * px
            };
            let h0_stroke = if h0_active { accent_gold } else { "#ffffff" };
            let h1_stroke = if h1_active { accent_gold } else { "#ffffff" };

            nodes.push(ui! {
                <circle cx={format!("{:.2}", c.c1.0)} cy={format!("{:.2}", c.c1.1)} r={format!("{hr0:.2}")} fill={stalk_col} stroke={h0_stroke} stroke-width={format!("{:.2}", 1.1 * px)} class="bezier-handle" />
            });
            nodes.push(ui! {
                <circle cx={format!("{:.2}", c.c2.0)} cy={format!("{:.2}", c.c2.1)} r={format!("{hr1:.2}")} fill={stalk_col} stroke={h1_stroke} stroke-width={format!("{:.2}", 1.1 * px)} class="bezier-handle" />
            });
        }

        // Sleek Corner & Curve Anchor Indicators (no giant ugly white blobs)
        let base_stroke = if p.is_selected {
            sel_stroke
        } else {
            unsel_stroke
        };
        let v_sw = if p.is_selected { 1.4 * px } else { 1.1 * px };

        for (pt_idx, &(pt_x, pt_y)) in p.points.iter().enumerate() {
            let prev_idx = (pt_idx + n_pts - 1) % n_pts;
            let has_incident_curve = p
                .curves
                .iter()
                .any(|c| c.edge_idx == pt_idx || c.edge_idx == prev_idx);

            let is_sel_pt = state
                .selected_2d_point
                .as_ref()
                .is_some_and(|(pid, pi)| pid == &p.id && *pi == pt_idx);
            let is_hov_pt = matches!(
                state.hovered_2d,
                Some(Hover2dTarget::Point { panel_idx, point_idx })
                    if panel_idx == p.panel_idx && point_idx == pt_idx
            );

            // Corner tool highlight ring on roundable vertices of active/hovered panel
            if corner_tool_active
                && (p.is_selected || is_hovered_panel || is_hov_pt)
                && let Some(edge_name) = p.edge_names.get(pt_idx)
                && max_fillet_radius(panel_ref, edge_name).is_ok_and(|mr| mr > 0.0005)
                && validate_fillet_preview(
                    template,
                    &p.id,
                    edge_name,
                    (state.corner_radius_mm / 1000.0).min(0.005),
                    CIRCLE_TENSION,
                )
                .is_ok()
            {
                nodes.push(ui! {
                    <circle cx={format!("{pt_x:.2}")} cy={format!("{pt_y:.2}")} r={format!("{:.2}", 5.0 * px)} fill="none" stroke={accent_emerald} stroke-width={format!("{:.2}", 1.2 * px)} />
                });
            }

            if is_sel_pt {
                nodes.push(ui! {
                    <circle cx={format!("{pt_x:.2}")} cy={format!("{pt_y:.2}")} r={format!("{:.2}", 6.4 * px)} fill="none" stroke={accent_gold} stroke-width={format!("{:.2}", 1.6 * px)} />
                });
            } else if is_hov_pt {
                nodes.push(ui! {
                    <circle cx={format!("{pt_x:.2}")} cy={format!("{pt_y:.2}")} r={format!("{:.2}", 5.5 * px)} fill="none" stroke={hover_cyan} stroke-width={format!("{:.2}", 1.3 * px)} />
                });
            }

            let v_fill = if is_sel_pt { accent_gold } else { vertex_fill };
            let v_stroke = if is_sel_pt { accent_gold } else { base_stroke };

            if has_incident_curve {
                // Smooth / curved anchor indicator (sleek small circle)
                let r = if is_sel_pt || is_hov_pt {
                    3.2 * px
                } else if p.is_selected {
                    2.5 * px
                } else {
                    1.9 * px
                };
                nodes.push(ui! {
                    <circle cx={format!("{pt_x:.2}")} cy={format!("{pt_y:.2}")} r={format!("{r:.2}")} fill={v_fill} stroke={v_stroke} stroke-width={format!("{v_sw:.2}")} class="cad-vertex" />
                });
            } else {
                // Sharp G0 CAD corner indicator (crisp small square)
                let half = if is_sel_pt || is_hov_pt {
                    3.0 * px
                } else if p.is_selected {
                    2.4 * px
                } else {
                    1.8 * px
                };
                nodes.push(ui! {
                    <rect x={format!("{:.2}", pt_x - half)} y={format!("{:.2}", pt_y - half)} width={format!("{:.2}", half * 2.0)} height={format!("{:.2}", half * 2.0)} fill={v_fill} stroke={v_stroke} stroke-width={format!("{v_sw:.2}")} class="cad-vertex" />
                });
            }
        }

        // Grainline with bidirectional arrowheads
        let rad = p.grain_deg.to_radians();
        let grain_len = ((p.max_x - p.min_x).min(p.max_y - p.min_y) * 0.28).max(40.0);
        let (cx, cy) = p.center;
        let gx0 = cx - grain_len * rad.sin();
        let gy0 = cy - grain_len * rad.cos();
        let gx1 = cx + grain_len * rad.sin();
        let gy1 = cy + grain_len * rad.cos();
        let g_pts = format!("{gx0:.2},{gy0:.2} {gx1:.2},{gy1:.2}");
        nodes.push(ui! {
            <polyline points={g_pts} fill="none" stroke={grain_color} stroke-width={format!("{:.2}", 1.2 * px)} />
        });
        let wing = 5.0 * px;
        let w_cos = rad.cos() * wing;
        let w_sin = rad.sin() * wing;
        let ah1 = format!(
            "{:.2},{:.2} {gx1:.2},{gy1:.2} {:.2},{:.2}",
            gx1 - w_sin - w_cos,
            gy1 - w_cos + w_sin,
            gx1 - w_sin + w_cos,
            gy1 - w_cos - w_sin
        );
        let ah0 = format!(
            "{:.2},{:.2} {gx0:.2},{gy0:.2} {:.2},{:.2}",
            gx0 + w_sin - w_cos,
            gy0 + w_cos + w_sin,
            gx0 + w_sin + w_cos,
            gy0 + w_cos - w_sin
        );
        nodes.push(ui! {
            <polyline points={ah1} fill="none" stroke={grain_color} stroke-width={format!("{:.2}", 1.2 * px)} />
        });
        nodes.push(ui! {
            <polyline points={ah0} fill="none" stroke={grain_color} stroke-width={format!("{:.2}", 1.2 * px)} />
        });

        // Clean center crosshair
        let ch_len = 5.5 * px;
        let ch_stroke = if p.is_selected {
            sel_stroke
        } else {
            grid_major
        };
        nodes.push(ui! {
            <polyline points={format!("{:.2},{cy:.2} {:.2},{cy:.2}", cx - ch_len, cx + ch_len)} fill="none" stroke={ch_stroke} stroke-width={format!("{:.2}", 1.0 * px)} />
        });
        nodes.push(ui! {
            <polyline points={format!("{cx:.2},{:.2} {cx:.2},{:.2}", cy - ch_len, cy + ch_len)} fill="none" stroke={ch_stroke} stroke-width={format!("{:.2}", 1.0 * px)} />
        });
    }

    // Live Corner Fillet Preview Overlay (when Corner tool is active on a valid corner)
    if let Some((panel_idx, _edge_name, preview, _eff_r)) = fillet_preview {
        if let Ok(arc_local) = curves::flatten(
            [
                preview.start,
                preview.control_1,
                preview.control_2,
                preview.end,
            ],
            curves::FLATNESS_M,
        ) {
            let arc_mm: Vec<(f64, f64)> = arc_local
                .into_iter()
                .map(|pt| to_mm(panel_to_board_pt(template, panel_idx, pt)))
                .collect();
            let arc_d = dashed_path_d(&arc_mm, 4.5 * px, 2.5 * px);
            if !arc_d.is_empty() {
                nodes.push(ui! {
                    <path d={arc_d} fill="none" stroke={accent_emerald} stroke-width={format!("{:.2}", 2.4 * px)} />
                });
            }
        }
        let s_mm = to_mm(panel_to_board_pt(template, panel_idx, preview.start));
        let e_mm = to_mm(panel_to_board_pt(template, panel_idx, preview.end));
        let c_mm = to_mm(panel_to_board_pt(template, panel_idx, preview.corner));
        let k_mm = to_mm(panel_to_board_pt(
            template,
            panel_idx,
            fillet_drag_handle_local(&preview),
        ));
        let guide_d = dashed_path_d(&[c_mm, k_mm], 3.0 * px, 2.5 * px);
        if !guide_d.is_empty() {
            nodes.push(ui! {
                <path d={guide_d} fill="none" stroke="rgba(16, 185, 129, 0.55)" stroke-width={format!("{:.2}", 1.0 * px)} />
            });
        }
        nodes.push(ui! {
            <circle cx={format!("{:.2}", s_mm.0)} cy={format!("{:.2}", s_mm.1)} r={format!("{:.2}", 2.8 * px)} fill={accent_emerald} stroke="#ffffff" stroke-width={format!("{:.2}", 0.9 * px)} />
        });
        nodes.push(ui! {
            <circle cx={format!("{:.2}", e_mm.0)} cy={format!("{:.2}", e_mm.1)} r={format!("{:.2}", 2.8 * px)} fill={accent_emerald} stroke="#ffffff" stroke-width={format!("{:.2}", 0.9 * px)} />
        });
        let knob_active = matches!(
            state.hovered_2d,
            Some(Hover2dTarget::CornerPreviewHandle { .. })
        ) || matches!(state.drag_2d, Some(Drag2d::CornerRadius { .. }));
        nodes.push(ui! {
            <circle cx={format!("{:.2}", k_mm.0)} cy={format!("{:.2}", k_mm.1)} r={format!("{:.2}", if knob_active { 5.4 * px } else { 4.4 * px })} fill={accent_emerald} stroke={if knob_active { accent_gold } else { "#ffffff" }} stroke-width={format!("{:.2}", 1.4 * px)} class="bezier-handle" />
        });
    }

    // Seam links between actual sewn boundary midpoints
    let edge_anchor_mm = |panel_id: &str, edge: Option<&str>, fallback: (f64, f64)| -> (f64, f64) {
        if let Some(edge_name) = edge
            && let Some(pidx) = panels.iter().position(|p| p.id == panel_id)
            && let Some(ref poly) = panels[pidx].polygon
            && let Some(eidx) = poly.edges.iter().position(|e| e == edge_name)
            && let Ok(pts) = poly.edge_points(eidx)
            && pts.len() >= 2
        {
            let total: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
            let mut target = total * 0.5;
            for pair in pts.windows(2) {
                let len = pair[0].distance(pair[1]);
                if len > 1e-12 && target <= len {
                    let mid = pair[0].lerp(pair[1], target / len);
                    return to_mm(panel_to_board_pt(template, pidx, mid));
                }
                target -= len;
            }
            return to_mm(panel_to_board_pt(template, pidx, pts[pts.len() / 2]));
        }
        fallback
    };

    for seam in &template.seams {
        let p_a = panel_geos.iter().find(|p| p.id == seam.a.panel);
        let p_b = panel_geos.iter().find(|p| p.id == seam.b.panel);
        if let (Some(a), Some(b)) = (p_a, p_b) {
            let pt_a = edge_anchor_mm(&a.id, seam.a.edge.as_deref(), a.center);
            let pt_b = edge_anchor_mm(&b.id, seam.b.edge.as_deref(), b.center);
            let d = dashed_path_d(&[pt_a, pt_b], 5.5 * px, 4.5 * px);
            if !d.is_empty() {
                nodes.push(ui! {
                    <path d={d} fill="none" stroke="rgba(16, 185, 129, 0.55)" stroke-width={format!("{:.2}", 1.1 * px)} />
                });
            }
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
        let _guard = worker::WORKER_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        // A click belongs to the instance that built the scene, even if another
        // instance advances first. An unrelated menu update must not reset the
        // switch's transition clock.
        let (other_tx, other_latest) = worker::start_cpu().expect("start second CPU worker");
        let other_files = DocumentFiles {
            output: dir.path().join("other"),
            source: None,
            modified: false,
        };
        let other = CssRunnerState::new(
            app.state().project.clone(),
            app.state().scene.clone(),
            other_files,
            other_tx,
            other_latest,
        );
        let mut other_app = create_runner_app(other, &stylesheet);
        other_app.set_viewport(cssimpler::renderer::ViewportSize::new(1600, 980));
        other_app.frame(FrameInfo {
            delta: Duration::ZERO,
            frame_index: 0,
        });
        let button = app.find_element_bounds_by_id("#theme-toggle").unwrap();
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        other_app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 1,
        });
        assert_eq!(other_app.state().theme, Theme::Dark);
        app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 1,
        });
        assert_eq!(app.state().theme, Theme::Light);
        assert!(app.latest_stats().transition_active);
        let elapsed = app.latest_stats().transition_elapsed_us;
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
            frame_index: 2,
        });
        assert!(app.state().solver_menu_open);
        assert!(app.latest_stats().transition_active);
        assert!(app.latest_stats().transition_elapsed_us > elapsed);
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
            frame_index: 3,
        });
        assert!(!app.state().solver_menu_open);

        // Repeated worker results must not prevent either theme endpoint.
        let dark_x = other_app
            .find_element_bounds_by_id("#theme-switch-thumb")
            .unwrap()
            .x;
        app.state_mut().playing = true;
        for i in 0..40 {
            let state = app.state();
            let done = worker::Done {
                generation: state.generation,
                had_grab: false,
                result: Ok((
                    state.scene.state.clone(),
                    VbdMetrics {
                        step_ms: i as f64 * 0.1,
                        ..VbdMetrics::default()
                    },
                )),
            };
            *state.worker_latest.lock().unwrap() = Some(done);
            app.frame(FrameInfo {
                delta: Duration::from_millis(16),
                frame_index: 100 + i,
            });
        }
        let light_x = app
            .find_element_bounds_by_id("#theme-switch-thumb")
            .unwrap()
            .x;
        assert!(
            (dark_x - light_x - 24.0).abs() < 0.1,
            "dark {dark_x}, light {light_x}"
        );
        let button = app.find_element_bounds_by_id("#theme-toggle").unwrap();
        assert!(cssimpler::renderer::dispatch_click(
            app.scene(),
            button.x + button.width / 2.0,
            button.y + button.height / 2.0
        ));
        for i in 0..40 {
            let state = app.state();
            let done = worker::Done {
                generation: state.generation,
                had_grab: false,
                result: Ok((
                    state.scene.state.clone(),
                    VbdMetrics {
                        step_ms: i as f64 * 0.1,
                        ..VbdMetrics::default()
                    },
                )),
            };
            *state.worker_latest.lock().unwrap() = Some(done);
            app.frame(FrameInfo {
                delta: Duration::from_millis(16),
                frame_index: 200 + i,
            });
        }
        app.state_mut().playing = false;
        app.state_mut().step_count = 0;
        let final_x = app
            .find_element_bounds_by_id("#theme-switch-thumb")
            .unwrap()
            .x;
        assert!(
            (dark_x - final_x).abs() < 0.1,
            "dark {dark_x}, final {final_x}"
        );

        // Structural controls must update even while simulation is playing.
        app.state_mut().playing = true;
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
            frame_index: 4,
        });
        assert!(app.state().solver_menu_open);
        assert!(app.find_element_bounds_by_id("#solver-dropdown").is_some());
        app.state_mut().playing = false;
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
            frame_index: 5,
        });
        assert!(!app.state().solver_menu_open);

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
        // Save the visible accepted checkpoint, not the launch-time project cache.
        app.state_mut().save_project();
        let saved =
            FittingProject::load(&app.state().files.output.join("fitting-project.json")).unwrap();
        let checkpoint = saved.state.unwrap();
        assert_eq!(checkpoint.x, app.state().scene.state.x);
        assert_eq!(checkpoint.v, app.state().scene.state.v);
        assert_eq!(checkpoint.time, app.state().scene.state.time);
        assert_eq!(saved.simulation.unwrap().dt, app.state().settings.dt);

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

    fn board_to_screen_px(state: &CssRunnerState, board_m: DVec2) -> (f32, f32) {
        let (_canvas, svg_rect) = compute_2d_viewport_rects(state);
        let (vbx, vby, span) = compute_2d_view_box_m(state);
        let u = ((board_m.x - vbx) / span) as f32;
        let v = ((board_m.y - vby) / span) as f32;
        (svg_rect.x + u * svg_rect.w, svg_rect.y + v * svg_rect.h)
    }

    fn pointer_move_ev(x: f32, y: f32) -> EngineEvent {
        EngineEvent::PointerMoved {
            position: cssimpler::renderer::PointerPosition { x, y },
            modifiers: cssimpler::renderer::KeyboardModifiers::default(),
        }
    }

    fn pointer_btn_ev(down: bool) -> EngineEvent {
        EngineEvent::PointerButton {
            button: cssimpler::renderer::PointerButton::Primary,
            state: if down {
                cssimpler::renderer::ButtonState::Pressed
            } else {
                cssimpler::renderer::ButtonState::Released
            },
            modifiers: cssimpler::renderer::KeyboardModifiers::default(),
        }
    }

    #[test]
    fn test_2d_svg_square_bounds_and_cad_bar() {
        let _guard = worker::WORKER_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let stylesheet = load_studio_stylesheet().expect("load studio stylesheet");
        let dir = tempfile::tempdir().expect("tempdir");
        let avatar = dir.path().join("avatar.obj");
        std::fs::write(&avatar, "v 0 0 -10\nv 1 0 -10\nv 0 1 -10\nf 1 2 3\n")
            .expect("write avatar");
        let project = FittingProject::basic(GarmentKind::Tshirt, 0, avatar);
        let scene = project.compile().expect("compile scene");
        let files = DocumentFiles {
            output: dir.path().join("out_square"),
            source: None,
            modified: false,
        };
        let (tx, latest) = worker::start_cpu().expect("start cpu worker");
        let state = CssRunnerState::new(project, scene, files, tx, latest);
        let mut app = create_runner_app(state, &stylesheet);

        for (w, h, mode, view_act) in [
            (1600, 980, ViewLayoutMode::Split, ACT_RUN_VIEW_SPLIT),
            (1600, 980, ViewLayoutMode::TwoDOnly, ACT_RUN_VIEW_2D),
            (1280, 800, ViewLayoutMode::Split, ACT_RUN_VIEW_SPLIT),
        ] {
            app.state_mut().window_size = (w as f32, h as f32);
            app.state_mut()
                .actions
                .fetch_or(view_act, Ordering::Relaxed);
            app.set_viewport(cssimpler::renderer::ViewportSize::new(w, h));
            app.frame(FrameInfo {
                delta: Duration::from_millis(16),
                frame_index: 1,
            });
            assert_eq!(app.state().layout_mode, mode);
            let svg_bounds = app
                .find_element_bounds_by_id("#svg-2d-board")
                .expect("#svg-2d-board bounds");
            let (_canvas_rect, expected_svg) = compute_2d_viewport_rects(app.state());
            assert!(
                (svg_bounds.width - svg_bounds.height).abs() < 1.5,
                "rendered #svg-2d-board must be square at {w}x{h} ({mode:?}): {svg_bounds:?}"
            );
            assert!(
                (svg_bounds.x - expected_svg.x).abs() < 3.0
                    && (svg_bounds.y - expected_svg.y).abs() < 3.0
                    && (svg_bounds.width - expected_svg.w).abs() < 3.0,
                "CSS layout ({svg_bounds:?}) must align with hit-test rect ({expected_svg:?}) at {w}x{h}"
            );
            let bar_bounds = app
                .find_element_bounds_by_id("#cad-2d-bar")
                .expect("#cad-2d-bar bounds");
            assert!(bar_bounds.width > 50.0 && bar_bounds.height >= 24.0);
        }
    }

    #[test]
    fn test_2d_canvas_click_corner_round_bezier_drag_and_undo() {
        let _guard = worker::WORKER_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().expect("tempdir");
        let avatar = dir.path().join("avatar.obj");
        std::fs::write(&avatar, "v 0 0 -10\nv 1 0 -10\nv 0 1 -10\nf 1 2 3\n")
            .expect("write avatar");
        let project = FittingProject::basic(GarmentKind::Tshirt, 0, avatar);
        let scene = project.compile().expect("compile scene");
        let files = DocumentFiles {
            output: dir.path().join("out_2d_edit"),
            source: None,
            modified: false,
        };
        let (tx, latest) = worker::start_cpu().expect("start cpu worker");
        let mut state = CssRunnerState::new(project, scene, files, tx, latest);
        assert_eq!(state.selected_piece_index, 0);

        // 1. Click interior of panel 1 ("back") -> switches selection to panel 1
        let back_center = DVec2::new(0.5, 0.35);
        let back_board = panel_to_board_pt(&state.project.template, 1, back_center);
        let (bx, by) = board_to_screen_px(&state, back_board);
        handle_runner_event(&mut state, &pointer_move_ev(bx, by));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(true)));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(false)));
        assert_eq!(state.selected_piece_index, 1);

        // 2. Click interior of panel 0 ("front") -> switches back to panel 0
        let front_center = DVec2::new(0.5, 0.35);
        let front_board = panel_to_board_pt(&state.project.template, 0, front_center);
        let (fx, fy) = board_to_screen_px(&state, front_board);
        handle_runner_event(&mut state, &pointer_move_ev(fx, fy));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(true)));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(false)));
        assert_eq!(state.selected_piece_index, 0);

        // 3. Click point #1 ("neck-bottom") -> selects corner sub-element
        let neck_pt = state.project.template.panels[0]
            .polygon
            .as_ref()
            .unwrap()
            .points[1];
        let neck_board = panel_to_board_pt(&state.project.template, 0, neck_pt);
        let (nx, ny) = board_to_screen_px(&state, neck_board);
        handle_runner_event(&mut state, &pointer_move_ev(nx, ny));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(true)));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(false)));
        assert_eq!(state.selected_2d_point, Some(("front".into(), 1)));

        // 4. Switch to Corner tool and click neck-bottom corner to round it
        state.corner_radius_mm = 35.0;
        state.active_2d_tool = "Corner".into();
        handle_runner_event(&mut state, &pointer_move_ev(nx, ny));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(true)));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(false)));

        let rounded_poly = state.project.template.panels[0].polygon.as_ref().unwrap();
        let curve_name = rounded_poly
            .curves
            .keys()
            .find(|k| k.starts_with("corner_round_"))
            .expect("Corner tool click must produce a corner_round_* Bézier curve")
            .clone();
        let initial_c1 = rounded_poly.curves[&curve_name][0];

        // 5. Render SVG and verify Bézier curve and handle elements appear
        let (svg_nodes, _) = render_runner_2d_svg(&state);
        fn count_class(nodes: &[Node], target: &str) -> usize {
            nodes
                .iter()
                .map(|n| match n {
                    Node::Element(el) => {
                        usize::from(el.classes.iter().any(|c| c == target))
                            + count_class(&el.children, target)
                    }
                    _ => 0,
                })
                .sum()
        }
        assert!(count_class(&svg_nodes, "bezier-handle") >= 2);
        assert!(count_class(&svg_nodes, "cad-vertex") >= 10);

        // 6. Drag Bézier handle 0 of the rounded corner and release -> commits AdjustCornerHandle
        state.active_2d_tool = "Edit".into();
        let h0_board = panel_to_board_pt(&state.project.template, 0, initial_c1);
        let (hx, hy) = board_to_screen_px(&state, h0_board);
        handle_runner_event(&mut state, &pointer_move_ev(hx, hy));
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(true)));
        handle_runner_event(&mut state, &pointer_move_ev(hx + 3.0, hy - 1.5));
        assert!(
            state.preview_template.is_some(),
            "live preview template must update while dragging Bézier handle"
        );
        assert!(handle_runner_event(&mut state, &pointer_btn_ev(false)));
        let updated_c1 = state.project.template.panels[0]
            .polygon
            .as_ref()
            .unwrap()
            .curves[&curve_name][0];
        assert!(
            updated_c1.distance(initial_c1) > 1e-4,
            "dragging Bézier handle must modify control point (initial={initial_c1:?}, updated={updated_c1:?}, status={:?})",
            state.status_message
        );

        // 7. Undo restores pre-drag handle and pre-round corner; Redo brings them back
        assert!(undo_redo_2d(&mut state, false));
        let undone_c1 = state.project.template.panels[0]
            .polygon
            .as_ref()
            .unwrap()
            .curves[&curve_name][0];
        assert!(undone_c1.distance(initial_c1) < 1e-9);
        assert!(undo_redo_2d(&mut state, false));
        assert!(
            state.project.template.panels[0]
                .polygon
                .as_ref()
                .unwrap()
                .curves
                .is_empty()
        );
        assert!(undo_redo_2d(&mut state, true));
        assert!(
            !state.project.template.panels[0]
                .polygon
                .as_ref()
                .unwrap()
                .curves
                .is_empty()
        );
    }
}

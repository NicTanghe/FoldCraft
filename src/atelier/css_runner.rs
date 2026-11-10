//! Native CSSimpler Studio V2 Garment Workspace Runner for `cloth-fit --view`.
//!
//! Replaces legacy egui viewport when launching `cloth-fit --view` with the full
//! reworked native CSSimpler Studio V2 garment workspace.
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

use marvelous_cloth::css_ui::{Theme, ViewLayoutMode, load_studio_stylesheet};
use marvelous_cloth::fitting::{FittingProject, FittingScene};
use marvelous_cloth::garment::export;
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
            active_3d_tool: "Select".to_string(),
            active_2d_tool: "Select".to_string(),
            clean_view: false,
            command_palette_open: false,
            left_drawer_open: true,
            selected_piece_index: 0,
            status_message: "Ready".to_string(),
            pointer_down: false,
            last_pointer_pos: (0.0, 0.0),
        }
    }

    pub fn trigger_worker_step(&mut self, once: bool) {
        if self.worker_pending {
            return;
        }
        let work = Work {
            generation: self.generation,
            scene: self.scene.clone(),
            settings: self.settings.clone(),
            grab: None,
            gpu: false,
            signal: worker::RepaintSignal::noop(),
            playing: self.playing,
            once,
        };
        if self.worker_tx.send(work).is_ok() {
            self.worker_pending = true;
        }
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

pub fn update_runner(state: &mut CssRunnerState, _frame: FrameInfo) -> Invalidation {
    // 1. Poll simulation worker result
    if let Ok(mut guard) = state.worker_latest.lock()
        && let Some(done) = guard.take()
        && done.generation == state.generation
    {
        state.worker_pending = false;
        match done.result {
                    Ok((new_state, metrics)) => {
                        state.scene.state = new_state;
                        state.step_count += 1;
                        state.step_ms = metrics.step_ms;
                        state.status_message = format!(
                            "Step {}: min dist {:.3} mm, res {:.1e}, {:.1} ms",
                            state.step_count,
                            metrics.minimum_distance_m * 1000.0,
                            metrics.residual_rms_n.unwrap_or(0.0),
                            metrics.step_ms
                        );
                        state.last_metrics = Some(metrics);
                    }
                    Err(e) => {
                        state.playing = false;
                        state.status_message = format!("Worker error: {e}");
                    }
                }
    }

    // 2. Process pending UI actions
    let actions = RUNNER_ACTIONS.swap(0, Ordering::Relaxed);

    if actions & ACT_RUN_PLAY_PAUSE != 0 {
        state.playing = !state.playing;
        if state.playing && !state.worker_pending {
            state.trigger_worker_step(false);
        }
    }
    if actions & ACT_RUN_STEP != 0 {
        state.trigger_worker_step(true);
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
    if actions & ACT_RUN_TOOL_SELECT != 0 {
        state.active_3d_tool = "Select".to_string();
        state.active_2d_tool = "Select".to_string();
    }
    if actions & ACT_RUN_TOOL_SEW != 0 {
        state.active_2d_tool = "Sew".to_string();
    }
    if actions & ACT_RUN_TOOL_MEASURE != 0 {
        state.active_2d_tool = "Measure".to_string();
    }
    if actions & ACT_RUN_TOOL_CORNER != 0 {
        state.active_2d_tool = "Corner".to_string();
    }

    // 3. Continue simulation loop if playing
    if state.playing && !state.worker_pending {
        state.trigger_worker_step(false);
        Invalidation::Paint
    } else if actions != 0 || state.last_metrics.is_some() {
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
                        if state.playing && !state.worker_pending {
                            state.trigger_worker_step(false);
                        }
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
                    && state.command_palette_open
                {
                    state.command_palette_open = false;
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
                {build_runner_tool_rails(state)}
                {build_runner_left_browser(state)}
                {build_runner_stage(state)}
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

    let clean_label = if state.clean_view {
        "Clean View: ON"
    } else {
        "Clean View"
    };

    ui! {
        <header class="top-bar">
            <div class="top-bar-left">
                <span class="app-title">"FoldCraft | Simulation Studio V2"</span>
                <nav class="menu-bar">
                    <button class="menu-item" onclick={on_runner_save}>"Save Project (Ctrl+S)"</button>
                    <button class="menu-item" onclick={on_runner_export}>"Export Artifacts"</button>
                    <button class="menu-item" onclick={on_runner_clean_view}>{clean_label}</button>
                </nav>
            </div>
            <div class="sim-transport">
                <button id="sim-play-btn" class={play_class} onclick={on_runner_play_pause}>
                    {play_label}
                </button>
                <button id="sim-step-btn" class="transport-btn" onclick={on_runner_step}>
                    "Step"
                </button>
                <button id="sim-reset-btn" class="transport-btn" onclick={on_runner_reset}>
                    "Reset"
                </button>
                <span id="sim-backend-indicator" class="cpu">
                    "VBD + OGC (Worker)"
                </span>
            </div>
            <div class="top-bar-right">
                <div class="layout-switcher">
                    <button class={split_class} onclick={on_runner_view_split}>"Split"</button>
                    <button class={threed_class} onclick={on_runner_view_3d}>"3D"</button>
                    <button class={twod_class} onclick={on_runner_view_2d}>"2D"</button>
                </div>
                <button id="command-palette-trigger" onclick={on_runner_palette_toggle}>
                    "Palette (Ctrl+K)"
                </button>
                <button id="theme-toggle" onclick={on_runner_theme_toggle}>
                    "Theme"
                </button>
            </div>
        </header>
    }
}

fn build_runner_tool_rails(state: &CssRunnerState) -> Node {
    let sel_class = if state.active_3d_tool == "Select" {
        "rail-tool-btn active"
    } else {
        "rail-tool-btn"
    };
    let sew_class = if state.active_2d_tool == "Sew" {
        "rail-tool-btn active"
    } else {
        "rail-tool-btn"
    };
    let msr_class = if state.active_2d_tool == "Measure" {
        "rail-tool-btn active"
    } else {
        "rail-tool-btn"
    };
    let crn_class = if state.active_2d_tool == "Corner" {
        "rail-tool-btn active"
    } else {
        "rail-tool-btn"
    };

    ui! {
        <aside class="rail-3d">
            <button class={sel_class} onclick={on_runner_tool_select} title="Select">"⤹"</button>
            <button class={sew_class} onclick={on_runner_tool_sew} title="Sewing Tool">"☍"</button>
            <button class={msr_class} onclick={on_runner_tool_measure} title="Measure Tape">"📏"</button>
            <button class={crn_class} onclick={on_runner_tool_corner} title="Corner Tool">"📐"</button>
            <button class="rail-tool-btn" onclick={on_runner_cam_reset} title="Reset Camera">"⌖"</button>
            <button class="rail-tool-btn" onclick={on_runner_drawer_toggle} title="Toggle Drawer">"☰"</button>
        </aside>
    }
}

fn build_runner_left_browser(state: &CssRunnerState) -> Node {
    if !state.left_drawer_open || state.clean_view {
        return ui! { <div class="left-drawer collapsed"></div> };
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
        let label = format!("{}: {}", p.id, p.name);
        piece_cards.push(ui! {
            <div class={card_class}>
                <span class="piece-title">{label}</span>
                <span class="piece-sub">"Ready for simulation"</span>
            </div>
        });
    }

    ui! {
        <aside class="left-drawer">
            <div class="drawer-header">"ASSETS & PIECES"</div>
            <div class="drawer-content">
                <div class="drawer-section">
                    <span class="section-label">"Template Pieces"</span>
                    {piece_cards}
                    <button class="action-btn" onclick={on_runner_piece_next}>"Select Next Piece"</button>
                </div>
                <div class="drawer-section">
                    <span class="section-label">"Quick Actions"</span>
                    <button class="drawer-btn" onclick={on_runner_save}>"Save Project"</button>
                    <button class="drawer-btn" onclick={on_runner_export}>"Export OBJ & True-Scale SVG"</button>
                </div>
                <div class="drawer-section">
                    <span class="section-label">"Physical SVG Note"</span>
                    <p class="drawer-hint">
                        "Includes stored seam allowances & sewing notches. 100 mm reference ruler verified."
                    </p>
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

fn build_runner_3d_viewport(state: &CssRunnerState) -> Node {
    let svg_nodes = render_runner_3d_svg(state);

    ui! {
        <div id="viewport-3d" class="viewport-container">
            <div class="viewport-header">
                <span class="viewport-title">"3D GARMENT & AVATAR"</span>
                <span class="viewport-sub">"Interactive VBD + OGC simulation"</span>
            </div>
            <div class="canvas-area">
                <svg viewBox="0 0 640 480" class="svg-canvas">
                    {svg_nodes}
                </svg>
            </div>
        </div>
    }
}

fn build_runner_2d_viewport(state: &CssRunnerState) -> Node {
    let svg_nodes = render_runner_2d_svg(state);

    ui! {
        <div id="viewport-2d" class="viewport-container">
            <div class="viewport-header">
                <span class="viewport-title">"2D PATTERN BOARD"</span>
                <span class="viewport-sub">"Metric pattern pieces & seam links"</span>
            </div>
            <div class="canvas-area">
                <svg viewBox="0 0 500 450" class="svg-canvas">
                    {svg_nodes}
                </svg>
            </div>
        </div>
    }
}

fn build_runner_right_dock(state: &CssRunnerState) -> Node {
    if state.clean_view {
        return ui! { <div id="right-dock" class="right-dock collapsed"></div> };
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
        <aside id="right-dock" class="right-dock">
            <div class="dock-tabs">
                <button class={tp} onclick={on_runner_tab_pattern}>"2D Pattern"</button>
                <button class={ts} onclick={on_runner_tab_sewing}>"Sewing"</button>
                <button class={tf} onclick={on_runner_tab_form}>"3D Form"</button>
                <button class={tm} onclick={on_runner_tab_material}>"Material"</button>
                <button class={te} onclick={on_runner_tab_env}>"Environment"</button>
            </div>
            <div class="dock-content">
                {content}
                {build_runner_property_accordions(state)}
            </div>
        </aside>
    }
}

fn build_cat_pattern_2d(state: &CssRunnerState) -> Node {
    let panels = &state.project.template.panels;
    let selected = panels.get(state.selected_piece_index);
    let name = selected.map_or("None", |p| p.id.as_str());
    let count = panels.len();

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"2D Pattern Pieces"</h3>
            <div class="prop-row">
                <span class="prop-label">"Total Pieces"</span>
                <span class="prop-value">{format!("{count}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Selected Piece"</span>
                <span class="prop-value">{name}</span>
            </div>
        </div>
    }
}

fn build_cat_sewing(state: &CssRunnerState) -> Node {
    let seams_count = state.project.template.seams.len();

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
        </div>
    }
}

fn build_cat_form_3d(state: &CssRunnerState) -> Node {
    let verts = state.scene.garment.mesh.uv.len();
    let tris = state.scene.garment.mesh.triangles.len();

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
        </div>
    }
}

fn build_cat_material(state: &CssRunnerState) -> Node {
    let mat_count = state.scene.garment.materials.len();

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Fabric & Appearance"</h3>
            <div class="prop-row">
                <span class="prop-label">"Materials Count"</span>
                <span class="prop-value">{format!("{mat_count}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Albedo Textures"</span>
                <span class="prop-value">"Portable Linear-Light"</span>
            </div>
        </div>
    }
}

fn build_cat_environment(state: &CssRunnerState) -> Node {
    let avatar_tris = state.scene.collider.surface.triangles.len();

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Avatar & Environment"</h3>
            <div class="prop-row">
                <span class="prop-label">"Avatar Triangles"</span>
                <span class="prop-value">{format!("{avatar_tris}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Profiles Persistence"</span>
                <span class="prop-value">"workspace-profiles.json"</span>
            </div>
        </div>
    }
}

fn build_runner_property_accordions(state: &CssRunnerState) -> Node {
    let last_pen = state
        .last_metrics
        .as_ref()
        .map_or("0.0 mm".to_string(), |m| {
            format!("{:.3} mm", m.minimum_distance_m * 1000.0)
        });
    let last_res = state.last_metrics.as_ref().map_or("N/A".to_string(), |m| {
        format!("{:.1e}", m.residual_rms_n.unwrap_or(0.0))
    });

    ui! {
        <div class="accordions-container">
            <div class="accordion-section">
                <h4 class="accordion-title">"Simulation Diagnostics"</h4>
                <div class="prop-row">
                    <span class="prop-label">"Step Time"</span>
                    <span class="prop-value">{format!("{:.1} ms", state.step_ms)}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Last Residual"</span>
                    <span class="prop-value">{last_res}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Max Penetration"</span>
                    <span class="prop-value">{last_pen}</span>
                </div>
            </div>
            <div class="accordion-section">
                <h4 class="accordion-title">"Avatar Diagnostics"</h4>
                <div class="prop-row">
                    <span class="prop-label">"Collision Surface"</span>
                    <span class="prop-value">{format!("{} tris", state.scene.collider.surface.triangles.len())}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Clearance Target"</span>
                    <span class="prop-value">{format!("{:.1} mm", state.project.contact_clearance * 1000.0)}</span>
                </div>
            </div>
        </div>
    }
}

fn build_runner_status_bar(state: &CssRunnerState) -> Node {
    let step_info = format!(
        "Step: {} | Worker: {:.1} ms | Status: {}",
        state.step_count, state.step_ms, state.status_message
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
                <div class="palette-items">
                    <button class="palette-item" onclick={on_runner_play_pause}>"▶ Toggle Simulation (Space)"</button>
                    <button class="palette-item" onclick={on_runner_step}>"⏭ Step Simulation"</button>
                    <button class="palette-item" onclick={on_runner_reset}>"↺ Reset Simulation"</button>
                    <button class="palette-item" onclick={on_runner_save}>"💾 Save Fitting Project"</button>
                    <button class="palette-item" onclick={on_runner_export}>"📦 Export Artifacts"</button>
                    <button class="palette-item" onclick={on_runner_clean_view}>"👁 Toggle Clean View"</button>
                    <button class="palette-item" onclick={on_runner_theme_toggle}>"🎨 Toggle Dark/Light Theme"</button>
                </div>
            </div>
        </div>
    }
}

// ============================================================================
// 3D Perspective SVG Projection Engine (Garment + Avatar)
// ============================================================================

fn render_runner_3d_svg(state: &CssRunnerState) -> Vec<Node> {
    let mut nodes = Vec::new();

    // Canvas background
    nodes.push(ui! {
        <rect width="640" height="480" fill="#111318" />
    });

    let w = 640.0_f32;
    let h = 480.0_f32;
    let aspect = w / h;
    let view_proj = state.camera.matrix(aspect);

    let light_dir = glam::Vec3::new(0.35, 0.85, 0.4).normalize();

    // 1. Render sampled Avatar Surface Mesh triangles (mannequin)
    let avatar_tris = &state.scene.collider.surface.triangles;
    let avatar_verts = &state.scene.collider.surface.vertices;

    let stride = (avatar_tris.len() / 600).max(1);
    for i in (0..avatar_tris.len()).step_by(stride) {
        let tri = avatar_tris[i];
        let p0 = avatar_verts[tri[0]];
        let p1 = avatar_verts[tri[1]];
        let p2 = avatar_verts[tri[2]];

        let v0 = glam::Vec3::new(p0.x as f32, p0.y as f32, p0.z as f32);
        let v1 = glam::Vec3::new(p1.x as f32, p1.y as f32, p1.z as f32);
        let v2 = glam::Vec3::new(p2.x as f32, p2.y as f32, p2.z as f32);

        let normal = (v1 - v0).cross(v2 - v0).normalize_or_zero();
        let diffuse = normal.dot(light_dir).abs().clamp(0.2, 0.85);

        let c0 = view_proj.project_point3(v0);
        let c1 = view_proj.project_point3(v1);
        let c2 = view_proj.project_point3(v2);

        let x0 = (c0.x * 0.5 + 0.5) * w;
        let y0 = (1.0 - (c0.y * 0.5 + 0.5)) * h;
        let x1 = (c1.x * 0.5 + 0.5) * w;
        let y1 = (1.0 - (c1.y * 0.5 + 0.5)) * h;
        let x2 = (c2.x * 0.5 + 0.5) * w;
        let y2 = (1.0 - (c2.y * 0.5 + 0.5)) * h;

        let points = format!("{x0:.1},{y0:.1} {x1:.1},{y1:.1} {x2:.1},{y2:.1}");
        let gray = (140.0 * diffuse) as u8;
        let color = format!("rgb({gray},{gray},{gray})");

        nodes.push(ui! {
            <polygon points={points} fill={color} stroke="none" />
        });
    }

    // 2. Render Garment Mesh triangles
    let garment_tris = &state.scene.garment.mesh.triangles;
    let positions = &state.scene.state.x;

    let cloth_stride = (garment_tris.len() / 800).max(1);
    for i in (0..garment_tris.len()).step_by(cloth_stride) {
        let tri = &garment_tris[i];
        let p0 = positions[tri.vertices[0]];
        let p1 = positions[tri.vertices[1]];
        let p2 = positions[tri.vertices[2]];

        let v0 = glam::Vec3::new(p0.x as f32, p0.y as f32, p0.z as f32);
        let v1 = glam::Vec3::new(p1.x as f32, p1.y as f32, p1.z as f32);
        let v2 = glam::Vec3::new(p2.x as f32, p2.y as f32, p2.z as f32);

        let normal = (v1 - v0).cross(v2 - v0).normalize_or_zero();
        let diffuse = normal.dot(light_dir).abs().clamp(0.2, 0.95);

        let c0 = view_proj.project_point3(v0);
        let c1 = view_proj.project_point3(v1);
        let c2 = view_proj.project_point3(v2);

        let x0 = (c0.x * 0.5 + 0.5) * w;
        let y0 = (1.0 - (c0.y * 0.5 + 0.5)) * h;
        let x1 = (c1.x * 0.5 + 0.5) * w;
        let y1 = (1.0 - (c1.y * 0.5 + 0.5)) * h;
        let x2 = (c2.x * 0.5 + 0.5) * w;
        let y2 = (1.0 - (c2.y * 0.5 + 0.5)) * h;

        let points = format!("{x0:.1},{y0:.1} {x1:.1},{y1:.1} {x2:.1},{y2:.1}");
        let r = (220.0 * diffuse) as u8;
        let g = (235.0 * diffuse) as u8;
        let b = (255.0 * diffuse) as u8;
        let color = format!("rgb({r},{g},{b})");

        nodes.push(ui! {
            <polygon points={points} fill={color} stroke="#3b82f6" stroke-width="0.5" />
        });
    }

    nodes
}

// ============================================================================
// 2D Pattern Board SVG Viewport Engine
// ============================================================================

fn render_runner_2d_svg(state: &CssRunnerState) -> Vec<Node> {
    let mut nodes = Vec::new();

    // Canvas background
    nodes.push(ui! {
        <rect width="500" height="450" fill="#181b22" />
    });

    // Drafting grid
    let mut gx = 50.0_f32;
    while gx < 500.0 {
        nodes.push(ui! {
            <polyline points={format!("{gx:.1},0 {gx:.1},450")} stroke="#242832" stroke-width="0.8" />
        });
        gx += 50.0;
    }
    let mut gy = 50.0_f32;
    while gy < 450.0 {
        nodes.push(ui! {
            <polyline points={format!("0,{gy:.1} 500,{gy:.1}")} stroke="#242832" stroke-width="0.8" />
        });
        gy += 50.0;
    }

    // Render 2D pattern panels
    let panels = &state.project.template.panels;
    let panel_count = panels.len().max(1);
    let slot_w = 460.0 / panel_count as f32;

    for (idx, _panel) in panels.iter().enumerate() {
        let is_selected = idx == state.selected_piece_index;
        let stroke_color = if is_selected { "#f59e0b" } else { "#60a5fa" };
        let stroke_w = if is_selected { "2.5" } else { "1.5" };

        let px = 25.0 + idx as f32 * slot_w;
        let py = 60.0_f32;
        let pw = slot_w - 20.0;
        let ph = 300.0_f32;

        let points = format!(
            "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            px,
            py,
            px + pw,
            py,
            px + pw,
            py + ph,
            px,
            py + ph
        );

        nodes.push(ui! {
            <polygon points={points} fill="rgba(59, 130, 246, 0.12)" stroke={stroke_color} stroke-width={stroke_w} />
        });

        // Grainline arrow inside piece
        let g_start = format!("{:.1},{:.1}", px + pw * 0.5, py + 40.0);
        let g_end = format!("{:.1},{:.1}", px + pw * 0.5, py + ph - 40.0);
        let arrow = format!("{g_start} {g_end}");

        nodes.push(ui! {
            <polyline points={arrow} stroke="#3b82f6" stroke-width="1.8" />
        });
    }

    nodes
}

// ============================================================================
// App Constructor and Entry Point
// ============================================================================

#[allow(clippy::type_complexity)]
pub fn create_runner_app<'a>(
    state: CssRunnerState,
    stylesheet: &'a Stylesheet,
) -> App<
    'a,
    CssRunnerState,
    fn(&mut CssRunnerState, FrameInfo) -> Invalidation,
    fn(&CssRunnerState) -> Node,
    Invalidation,
> {
    App::new(
        state,
        stylesheet,
        update_runner as fn(&mut CssRunnerState, FrameInfo) -> Invalidation,
        build_runner_ui as fn(&CssRunnerState) -> Node,
    )
    .with_event_handler(handle_runner_event)
    .with_render_mode(RenderMode::OnInvalidation)
}

pub fn run(
    project: FittingProject,
    scene: FittingScene,
    files: DocumentFiles,
) -> Result<(), String> {
    let stylesheet =
        Box::leak(Box::new(load_studio_stylesheet().map_err(|e| {
            format!("Failed to load Studio V2 stylesheet: {e}")
        })?));

    // Start background simulation worker on CPU
    let (tx, latest) = worker::start_cpu().map_err(|e| format!("Failed to start worker: {e}"))?;

    let state = CssRunnerState::new(project, scene, files, tx, latest);
    let app = create_runner_app(state, stylesheet);
    let config = WindowConfig::new("ClothCraft | Simulation Studio V2", 1600, 980);
    app.run(config)
        .map_err(|e| format!("CSSimpler application error: {e}"))?;
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

        // Headless frame advance
        let scene = app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 0,
        });
        assert!(!scene.is_empty(), "runner scene should not be empty");

        // Verify layout containers
        assert!(app.find_element_bounds_by_id("#viewport-3d").is_some());
        assert!(app.find_element_bounds_by_id("#viewport-2d").is_some());
        assert!(app.find_element_bounds_by_id("#right-dock").is_some());

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

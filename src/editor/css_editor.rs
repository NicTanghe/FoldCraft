//! Native CSSimpler Project v1 workshop with an independent physics worker.
//!
//! Replaces plain `cargo run` (`editor::run`) with a native `cssimpler::app::App`
//! Studio V2 window backed by Project v1, WGSL AVBD / IPC (default), the explicit
//! CPU Newton control, and mesh preparation.

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use cssimpler::app::{App, Invalidation, RenderMode};
use cssimpler::core::Node;
use cssimpler::renderer::{EngineEvent, FrameInfo, WindowConfig};
use cssimpler::style::Stylesheet;
use cssimpler::ui;

use marvelous_cloth::css_ui::{Theme, ViewLayoutMode, load_studio_stylesheet};
use marvelous_cloth::mesh_resolution::preparation::PhysicsSizing;
use marvelous_cloth::model::{Mesh, Scene, State};
use marvelous_cloth::project::{self, Project};
use marvelous_cloth::render::Camera;
use marvelous_cloth::solver::{Metrics, ReferenceSolver};

// ============================================================================
// Actions bitmask for atomic dispatch from ui! onclick callbacks
// ============================================================================

pub const ACT_PLAY_PAUSE: u64 = 1 << 0;
pub const ACT_STEP: u64 = 1 << 1;
pub const ACT_RESET: u64 = 1 << 2;
pub const ACT_SAVE: u64 = 1 << 3;
pub const ACT_EXPORT_OBJ: u64 = 1 << 4;
pub const ACT_REMESH_RESET: u64 = 1 << 5;
pub const ACT_UNIFORM_RESET: u64 = 1 << 6;
pub const ACT_SCENE_SPHERE: u64 = 1 << 7;
pub const ACT_SCENE_HANG: u64 = 1 << 8;
pub const ACT_TOGGLE_WIRE: u64 = 1 << 9;
pub const ACT_TOGGLE_SHADED: u64 = 1 << 10;
pub const ACT_TOGGLE_COLLIDER: u64 = 1 << 11;
pub const ACT_TAB_PROPERTIES: u64 = 1 << 12;
pub const ACT_TAB_OBJECTS: u64 = 1 << 13;
pub const ACT_TAB_FABRIC: u64 = 1 << 14;
pub const ACT_TAB_SIMULATION: u64 = 1 << 15;
pub const ACT_THEME_TOGGLE: u64 = 1 << 16;
pub const ACT_VIEW_SPLIT: u64 = 1 << 17;
pub const ACT_VIEW_3D: u64 = 1 << 18;
pub const ACT_VIEW_2D: u64 = 1 << 19;
pub const ACT_PALETTE_TOGGLE: u64 = 1 << 20;
pub const ACT_DRAWER_TOGGLE: u64 = 1 << 21;
pub const ACT_CAM_RESET: u64 = 1 << 22;
pub const ACT_EDGE_SELECT_NEXT: u64 = 1 << 23;

static PENDING_ACTIONS: AtomicU64 = AtomicU64::new(0);

pub fn dispatch_action(act: u64) {
    PENDING_ACTIONS.fetch_or(act, Ordering::Relaxed);
}

// Onclick trampolines
pub fn on_play_pause() {
    dispatch_action(ACT_PLAY_PAUSE);
}
pub fn on_step() {
    dispatch_action(ACT_STEP);
}
pub fn on_reset() {
    dispatch_action(ACT_RESET);
}
pub fn on_save() {
    dispatch_action(ACT_SAVE);
}
pub fn on_export_obj() {
    dispatch_action(ACT_EXPORT_OBJ);
}
pub fn on_remesh_reset() {
    dispatch_action(ACT_REMESH_RESET);
}
pub fn on_uniform_reset() {
    dispatch_action(ACT_UNIFORM_RESET);
}
pub fn on_scene_sphere() {
    dispatch_action(ACT_SCENE_SPHERE);
}
pub fn on_scene_hang() {
    dispatch_action(ACT_SCENE_HANG);
}
pub fn on_toggle_wire() {
    dispatch_action(ACT_TOGGLE_WIRE);
}
pub fn on_toggle_shaded() {
    dispatch_action(ACT_TOGGLE_SHADED);
}
pub fn on_toggle_collider() {
    dispatch_action(ACT_TOGGLE_COLLIDER);
}
pub fn on_tab_properties() {
    dispatch_action(ACT_TAB_PROPERTIES);
}
pub fn on_tab_objects() {
    dispatch_action(ACT_TAB_OBJECTS);
}
pub fn on_tab_fabric() {
    dispatch_action(ACT_TAB_FABRIC);
}
pub fn on_tab_simulation() {
    dispatch_action(ACT_TAB_SIMULATION);
}
pub fn on_theme_toggle() {
    dispatch_action(ACT_THEME_TOGGLE);
}
pub fn on_view_split() {
    dispatch_action(ACT_VIEW_SPLIT);
}
pub fn on_view_3d() {
    dispatch_action(ACT_VIEW_3D);
}
pub fn on_view_2d() {
    dispatch_action(ACT_VIEW_2D);
}
pub fn on_palette_toggle() {
    dispatch_action(ACT_PALETTE_TOGGLE);
}
pub fn on_drawer_toggle() {
    dispatch_action(ACT_DRAWER_TOGGLE);
}
pub fn on_cam_reset() {
    dispatch_action(ACT_CAM_RESET);
}
pub fn on_edge_select_next() {
    dispatch_action(ACT_EDGE_SELECT_NEXT);
}

// ============================================================================
// Editor Tab Enum
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EditorTab {
    #[default]
    Properties,
    Objects,
    Fabric,
    Simulation,
}

// ============================================================================
// ============================================================================
// Reference Simulation Worker
// ============================================================================

pub struct RefWork {
    pub generation: u64,
    pub mesh: Arc<Mesh>,
    pub state: State,
    pub material: marvelous_cloth::model::Material,
    pub solver: marvelous_cloth::solver::SolverSettings,
    pub sphere: Option<marvelous_cloth::contact::Sphere>,
    pub playing: bool,
    pub once: bool,
}

pub enum EditorMetrics {
    Reference(Box<Metrics>),
    Avbd(Box<marvelous_cloth::vbd::VbdMetrics>),
}

pub struct RefDone {
    pub generation: u64,
    pub result: Result<(State, EditorMetrics), String>,
}

pub type RefLatest = Arc<Mutex<Option<RefDone>>>;

pub fn start_reference_worker() -> Result<(mpsc::Sender<RefWork>, RefLatest), std::io::Error> {
    start_editor_worker(None)
}

fn start_editor_worker(
    mut gpu: Option<marvelous_cloth::gpu_avbd::ProjectAvbd>,
) -> Result<(mpsc::Sender<RefWork>, RefLatest), std::io::Error> {
    let (tx, rx) = mpsc::channel::<RefWork>();
    let latest: RefLatest = Arc::new(Mutex::new(None));
    let output = latest.clone();
    std::thread::Builder::new()
        .name("cloth-reference".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let mut active: Option<RefWork> = None;
            let mut deadline = Instant::now();
            let mut generation = None;
            loop {
                let wait = active.as_ref().and_then(|w| {
                    if w.once {
                        Some(std::time::Duration::ZERO)
                    } else if w.playing {
                        Some(deadline.saturating_duration_since(Instant::now()))
                    } else {
                        None
                    }
                });
                let incoming = match wait {
                    Some(dur) => match rx.recv_timeout(dur) {
                        Ok(work) => Some(work),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    },
                    None => match rx.recv() {
                        Ok(work) => Some(work),
                        Err(_) => break,
                    },
                };
                if let Some(work) = incoming {
                    accept_ref(&mut active, work, &mut deadline);
                    while let Ok(work) = rx.try_recv() {
                        accept_ref(&mut active, work, &mut deadline);
                    }
                }
                let Some(work) = active.as_mut() else {
                    continue;
                };
                if !work.once && (!work.playing || Instant::now() < deadline) {
                    continue;
                }
                let was_once = work.once;
                work.once = false;

                if generation != Some(work.generation) {
                    if let Some(gpu) = &mut gpu {
                        gpu.reset();
                    }
                    generation = Some(work.generation);
                }
                let outcome = if let Some(gpu) = &mut gpu {
                    let project = Project {
                        material: work.material.clone(),
                        solver: work.solver.clone(),
                        sphere: work.sphere.clone(),
                        ..Default::default()
                    };
                    gpu.step(&project, &work.mesh, &mut work.state)
                        .map(|m| EditorMetrics::Avbd(Box::new(m)))
                } else {
                    ReferenceSolver::step(
                        &work.mesh,
                        &mut work.state,
                        &work.material,
                        &work.solver,
                        work.sphere.as_ref(),
                    )
                    .map(|m| EditorMetrics::Reference(Box::new(m)))
                }
                .map(|m| (work.state.clone(), m));
                if outcome.is_err() {
                    work.playing = false;
                }

                let generation = work.generation;
                if let Ok(mut slot) = output.lock() {
                    *slot = Some(RefDone {
                        generation,
                        result: outcome,
                    });
                }
                if !was_once {
                    let dt = work.solver.dt.max(0.001);
                    let dt = std::time::Duration::from_secs_f64(dt);
                    deadline = (deadline + dt).max(Instant::now() - dt * 4);
                }
            }
        })?;
    Ok((tx, latest))
}

fn accept_ref(active: &mut Option<RefWork>, mut new: RefWork, deadline: &mut Instant) {
    if let Some(old) = active {
        if old.generation == new.generation {
            std::mem::swap(&mut old.state, &mut new.state);
            new.once |= old.once;
            if new.playing && !old.playing {
                *deadline = Instant::now();
            }
        } else {
            *deadline = Instant::now();
        }
    }
    *active = Some(new);
}

// ============================================================================
// State Struct
// ============================================================================

#[derive(Clone)]
pub struct CssEditorState {
    pub usd_scene: Option<cssimpler::renderer::gpu::usd::UsdScene>,
    pub active: Project,
    pub mesh: Arc<Mesh>,
    pub state: State,
    pub metrics: Option<Metrics>,
    pub gpu_sim: bool,
    pub camera: Camera,
    pub playing: bool,
    pub wire: bool,
    pub shaded: bool,
    pub collider: bool,
    pub step_count: u64,
    pub step_ms: f64,
    pub output_dir: PathBuf,
    pub active_tab: EditorTab,
    pub theme: Theme,
    pub layout_mode: ViewLayoutMode,
    pub command_palette_open: bool,
    pub left_drawer_open: bool,
    pub selected_edge: Option<usize>,
    pub status_message: String,
    // Mesh prep inputs
    pub mesh_prep_warp_e: String,
    pub mesh_prep_weft_e: String,
    pub mesh_prep_ratio: String,
    pub mesh_prep_transition: String,
    pub mesh_prep_error: Option<String>,
    // Pointer / camera drag state
    pub pointer_down: bool,
    pub last_pointer_pos: (f32, f32),
    pub worker_tx: mpsc::Sender<RefWork>,
    pub worker_latest: RefLatest,
    pub generation: u64,
}

impl CssEditorState {
    pub fn new(project: Project, output_dir: PathBuf) -> Self {
        Self::new_with_backend(project, output_dir, None)
    }

    fn new_with_backend(
        project: Project,
        output_dir: PathBuf,
        gpu: Option<marvelous_cloth::gpu_avbd::ProjectAvbd>,
    ) -> Self {
        let gpu_sim = gpu.is_some();
        let (mesh, state) = project.build().expect("build initial mesh from project");
        let (warp, weft, ratio, trans) = if let Some(plan) = &project.physics_sizing {
            (
                plan.young_pa[0].to_string(),
                plan.young_pa[1].to_string(),
                plan.boundary_ratio.to_string(),
                (plan.transition_m * 1000.0).to_string(),
            )
        } else {
            (
                "110000".to_string(),
                "110000".to_string(),
                "0.5".to_string(),
                "40".to_string(),
            )
        };

        let (worker_tx, worker_latest) = start_editor_worker(gpu).expect("start physics worker");

        Self {
            usd_scene: None,
            active: project,
            mesh: Arc::new(mesh),
            state,
            metrics: None,
            gpu_sim,
            camera: Camera::default(),
            playing: false,
            wire: true,
            shaded: true,
            collider: true,
            step_count: 0,
            step_ms: 0.0,
            output_dir,
            active_tab: EditorTab::Properties,
            theme: Theme::Dark,
            layout_mode: ViewLayoutMode::Split,
            command_palette_open: false,
            left_drawer_open: true,
            selected_edge: None,
            status_message: "Ready".to_string(),
            mesh_prep_warp_e: warp,
            mesh_prep_weft_e: weft,
            mesh_prep_ratio: ratio,
            mesh_prep_transition: trans,
            mesh_prep_error: None,
            pointer_down: false,
            last_pointer_pos: (0.0, 0.0),
            worker_tx,
            worker_latest,
            generation: 1,
        }
    }

    pub fn send_control_state(&mut self, once: bool) {
        let work = RefWork {
            generation: self.generation,
            mesh: self.mesh.clone(),
            state: self.state.clone(),
            material: self.active.material.clone(),
            solver: self.active.solver.clone(),
            sphere: self.active.sphere.clone(),
            playing: self.playing,
            once,
        };
        let _ = self.worker_tx.send(work);
    }

    pub fn trigger_step(&mut self) {
        self.send_control_state(true);
    }

    pub fn step(&mut self) {
        self.trigger_step();
        let deadline = Instant::now() + std::time::Duration::from_millis(1000);
        while Instant::now() < deadline {
            if let Some(done) = self.worker_latest.lock().ok().and_then(|mut g| g.take())
                && done.generation == self.generation
                && let Ok((new_state, m)) = done.result
            {
                self.accept_step(new_state, m);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn accept_step(&mut self, new_state: State, metrics: EditorMetrics) {
        self.state = new_state;
        self.step_count += 1;
        match metrics {
            EditorMetrics::Reference(m) => {
                self.step_ms = m.step_ms;
                self.status_message = format!(
                    "Step {}: max pen {:.3} mm, residual {:.1e}, {:.1} ms",
                    self.step_count,
                    m.max_vertex_penetration_m * 1000.,
                    m.residual_rms_n,
                    m.step_ms
                );
                self.metrics = Some(*m);
            }
            EditorMetrics::Avbd(m) => {
                self.step_ms = m.step_ms;
                self.metrics = None;
                self.status_message = format!(
                    "WGSL AVBD / IPC step {}: {:.1} ms; fixed budget, residual unavailable",
                    self.step_count, m.step_ms
                );
            }
        }
        self.sync_usd_points();
    }

    pub fn sync_usd_points(&mut self) {
        if let Some(scene) = &self.usd_scene
            && let Err(error) = marvelous_cloth::usd_scene::update_viewport_points(
                scene,
                "/World/Garment/cloth_mesh",
                &self.state.x,
            )
        {
            self.status_message = format!("USD update error: {error}");
        }
    }

    pub fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.playing = false;
        if let Ok((mesh, state)) = self.active.build() {
            self.mesh = Arc::new(mesh);
            self.state = state;
            self.metrics = None;
            self.step_count = 0;
            self.step_ms = 0.0;
            self.status_message = "Reset simulation state".to_string();
            if let Some(scene) = &self.usd_scene
                && let Err(error) =
                    marvelous_cloth::usd_scene::update_viewport_topology(scene, &self.mesh)
            {
                self.status_message = format!("USD topology error: {error}");
            }
            self.sync_usd_points();
            self.send_control_state(false);
        }
    }

    pub fn save_project(&mut self) {
        let _ = fs::create_dir_all(&self.output_dir);
        let path = self.output_dir.join("project.json");
        match self.active.save(&path) {
            Ok(_) => self.status_message = format!("Saved {}", path.display()),
            Err(e) => self.status_message = format!("Save error: {e}"),
        }
    }

    pub fn export_obj(&mut self) {
        let _ = fs::create_dir_all(&self.output_dir);
        let path = self.output_dir.join("cloth.obj");
        let obj_text = project::obj(&self.mesh, &self.state);
        match fs::write(&path, obj_text) {
            Ok(_) => self.status_message = format!("Exported {}", path.display()),
            Err(e) => self.status_message = format!("Export error: {e}"),
        }
    }

    pub fn remesh_and_reset(&mut self) {
        let parse_num = |s: &str| {
            s.trim()
                .parse::<f64>()
                .map_err(|_| "Enter numeric sizing values".to_string())
        };

        let result = (|| -> Result<PhysicsSizing, String> {
            let warp = parse_num(&self.mesh_prep_warp_e)?;
            let weft = parse_num(&self.mesh_prep_weft_e)?;
            let ratio = parse_num(&self.mesh_prep_ratio)?;
            let trans_mm = parse_num(&self.mesh_prep_transition)?;
            let plan = PhysicsSizing {
                young_pa: [warp, weft],
                bending_nm: [
                    self.active.material.bend_warp,
                    self.active.material.bend_weft,
                ],
                axis_degrees: self.active.material.grain_degrees,
                boundary_ratio: ratio,
                transition_m: trans_mm * 0.001,
                seed: 42,
            };
            plan.validate()?;
            Ok(plan)
        })();

        match result {
            Ok(plan) => {
                self.active.physics_sizing = Some(plan);
                self.mesh_prep_error = None;
                self.reset();
                self.status_message = "Remeshed with physics sizing".to_string();
            }
            Err(e) => {
                self.mesh_prep_error = Some(e);
            }
        }
    }

    pub fn uniform_grid_and_reset(&mut self) {
        self.active.physics_sizing = None;
        self.mesh_prep_error = None;
        self.reset();
        self.status_message = "Reset to uniform grid".to_string();
    }
}

// ============================================================================
// Update and Event Handling
// ============================================================================

pub fn update_editor(state: &mut CssEditorState, _frame: FrameInfo) -> Invalidation {
    let done = state
        .worker_latest
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    let mut had_result = false;
    if let Some(done) = done
        && done.generation == state.generation
    {
        match done.result {
            Ok((new_state, m)) => {
                had_result = true;
                state.accept_step(new_state, m);
            }
            Err(e) => {
                state.playing = false;
                state.status_message = format!("Solver error: {e}");
                state.send_control_state(false);
            }
        }
    }

    let actions = PENDING_ACTIONS.swap(0, Ordering::Relaxed);

    if actions & ACT_PLAY_PAUSE != 0 {
        state.playing = !state.playing;
        state.send_control_state(false);
    }
    if actions & ACT_STEP != 0 {
        state.trigger_step();
    }
    if actions & ACT_RESET != 0 {
        state.reset();
    }
    if actions & ACT_SAVE != 0 {
        state.save_project();
    }
    if actions & ACT_EXPORT_OBJ != 0 {
        state.export_obj();
    }
    if actions & ACT_REMESH_RESET != 0 {
        state.remesh_and_reset();
    }
    if actions & ACT_UNIFORM_RESET != 0 {
        state.uniform_grid_and_reset();
    }
    if actions & ACT_SCENE_SPHERE != 0 {
        state.active.scene = Scene::Sphere;
        state.active.sphere = Some(Default::default());
        state.reset();
    }
    if actions & ACT_SCENE_HANG != 0 {
        state.active.scene = Scene::Hanging;
        state.active.sphere = None;
        state.reset();
    }
    if actions & ACT_TOGGLE_WIRE != 0 {
        state.wire = !state.wire;
    }
    if actions & ACT_TOGGLE_SHADED != 0 {
        state.shaded = !state.shaded;
    }
    if actions & ACT_TOGGLE_COLLIDER != 0 {
        state.collider = !state.collider;
    }
    if actions & ACT_TAB_PROPERTIES != 0 {
        state.active_tab = EditorTab::Properties;
    }
    if actions & ACT_TAB_OBJECTS != 0 {
        state.active_tab = EditorTab::Objects;
    }
    if actions & ACT_TAB_FABRIC != 0 {
        state.active_tab = EditorTab::Fabric;
    }
    if actions & ACT_TAB_SIMULATION != 0 {
        state.active_tab = EditorTab::Simulation;
    }
    if actions & ACT_THEME_TOGGLE != 0 {
        state.theme = state.theme.toggled();
    }
    if actions & ACT_VIEW_SPLIT != 0 {
        state.layout_mode = ViewLayoutMode::Split;
    }
    if actions & ACT_VIEW_3D != 0 {
        state.layout_mode = ViewLayoutMode::ThreeDOnly;
    }
    if actions & ACT_VIEW_2D != 0 {
        state.layout_mode = ViewLayoutMode::TwoDOnly;
    }
    if actions & ACT_PALETTE_TOGGLE != 0 {
        state.command_palette_open = !state.command_palette_open;
    }
    if actions & ACT_DRAWER_TOGGLE != 0 {
        state.left_drawer_open = !state.left_drawer_open;
    }
    if actions & ACT_CAM_RESET != 0 {
        state.camera = Camera::default();
    }
    if actions & ACT_EDGE_SELECT_NEXT != 0 {
        state.selected_edge = Some(state.selected_edge.map_or(0, |e| (e + 1) % 4));
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

pub fn handle_editor_event(state: &mut CssEditorState, event: &EngineEvent) -> bool {
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
                // If dragging in 3D viewport region, orbit camera
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
            state.camera.distance = (state.camera.distance - dy).clamp(0.5, 12.0);
            false
        }
        _ => false,
    }
}

// ============================================================================
// UI Generation (Studio V2 layout via cssimpler::ui!)
// ============================================================================

pub fn build_editor_ui(state: &CssEditorState) -> Node {
    let theme_class = state.theme.class_name();
    let shell_class = format!("studio-shell {theme_class}");

    ui! {
        <div id="app" class={shell_class}>
            {build_top_bar(state)}
            <div class="studio-body">
                {build_tool_rail_3d(state)}
                {build_left_drawer(state)}
                {build_stage(state)}
                {build_right_dock(state)}
            </div>
            {build_status_bar(state)}
            {build_command_palette_overlay(state)}
        </div>
    }
}

fn build_top_bar(state: &CssEditorState) -> Node {
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

    ui! {
        <header class="top-bar">
            <div class="top-bar-left">
                <span class="app-title">"FoldCraft | Reference Editor"</span>
                <nav class="menu-bar">
                    <button class="menu-item" onclick={on_save}>"Save (Ctrl+S)"</button>
                    <button class="menu-item" onclick={on_export_obj}>"Export OBJ"</button>
                </nav>
            </div>
            <div class="sim-transport">
                <button id="sim-play-btn" class={play_class} onclick={on_play_pause}>
                    {play_label}
                </button>
                <button id="sim-step-btn" class="transport-btn" onclick={on_step}>
                    "Step"
                </button>
                <button id="sim-reset-btn" class="transport-btn" onclick={on_reset}>
                    "Reset"
                </button>
                <span id="sim-backend-indicator" class="cpu">
                    {if state.gpu_sim {"WGSL AVBD / IPC"} else {"CPU Reference"}}
                </span>
            </div>
            <div class="top-bar-right">
                <div class="layout-switcher">
                    <button class={split_class} onclick={on_view_split}>"Split"</button>
                    <button class={threed_class} onclick={on_view_3d}>"3D"</button>
                    <button class={twod_class} onclick={on_view_2d}>"2D"</button>
                </div>
                <button id="command-palette-trigger" onclick={on_palette_toggle}>
                    "Palette (Ctrl+K)"
                </button>
                <button id="theme-toggle" onclick={on_theme_toggle}>
                    "Theme"
                </button>
            </div>
        </header>
    }
}

fn build_tool_rail_3d(_state: &CssEditorState) -> Node {
    ui! {
        <aside class="rail-3d">
            <button class="rail-tool-btn active" title="Select / Orbit">"⤹"</button>
            <button class="rail-tool-btn" onclick={on_cam_reset} title="Reset Camera">"⌖"</button>
            <button class="rail-tool-btn" onclick={on_edge_select_next} title="Cycle Selected Edge">"✂"</button>
            <button class="rail-tool-btn" onclick={on_drawer_toggle} title="Toggle Left Drawer">"☰"</button>
        </aside>
    }
}

fn build_left_drawer(state: &CssEditorState) -> Node {
    if !state.left_drawer_open {
        return ui! { <div class="left-drawer collapsed"></div> };
    }

    let is_sphere = state.active.scene == Scene::Sphere;
    let sphere_class = if is_sphere {
        "drawer-btn active"
    } else {
        "drawer-btn"
    };
    let hang_class = if !is_sphere {
        "drawer-btn active"
    } else {
        "drawer-btn"
    };

    let wire_label = if state.wire {
        "Wireframe: ON"
    } else {
        "Wireframe: OFF"
    };
    let shaded_label = if state.shaded {
        "Shaded: ON"
    } else {
        "Shaded: OFF"
    };
    let collider_label = if state.collider {
        "Collider: ON"
    } else {
        "Collider: OFF"
    };

    ui! {
        <aside class="left-drawer">
            <div class="drawer-header">"SCENE & DISPLAY"</div>
            <div class="drawer-content">
                <div class="drawer-section">
                    <span class="section-label">"Reference Benchmark Scene"</span>
                    <button id="scene-sphere-btn" class={sphere_class} onclick={on_scene_sphere}>
                        "Sphere Benchmark"
                    </button>
                    <button id="scene-hang-btn" class={hang_class} onclick={on_scene_hang}>
                        "Hanging Pins"
                    </button>
                </div>
                <div class="drawer-section">
                    <span class="section-label">"Display Toggles"</span>
                    <button class="drawer-btn" onclick={on_toggle_wire}>{wire_label}</button>
                    <button class="drawer-btn" onclick={on_toggle_shaded}>{shaded_label}</button>
                    <button class="drawer-btn" onclick={on_toggle_collider}>{collider_label}</button>
                </div>
                <div class="drawer-section">
                    <span class="section-label">"Camera Orbit Hint"</span>
                    <p class="drawer-hint">"Drag mouse on 3D viewport to orbit. Mouse wheel to zoom."</p>
                </div>
            </div>
        </aside>
    }
}

fn build_stage(state: &CssEditorState) -> Node {
    let stage_class = format!("stage {}", state.layout_mode.stage_class());

    ui! {
        <main id="viewport-stage" class={stage_class}>
            {build_3d_viewport(state)}
            {build_2d_viewport(state)}
        </main>
    }
}

fn build_3d_viewport(_state: &CssEditorState) -> Node {
    ui! {
        <div id="viewport-3d" class="viewport-container">
            <div class="viewport-header">
                <span class="viewport-title">"3D CLOTH VIEWPORT"</span>
                <span class="viewport-sub">"USD native 3D viewport"</span>
            </div>
            <div class="canvas-area">
                <div id="viewport-3d-scene" class="viewport"></div>
            </div>
        </div>
    }
}

fn build_2d_viewport(state: &CssEditorState) -> Node {
    let svg_nodes = render_2d_pattern_svg(state);
    let edge_hint = state
        .selected_edge
        .map_or("Click edge to select".to_string(), |e| {
            let p = &state.active.pattern;
            let mm = if e % 2 == 0 {
                p.width * 1000.0
            } else {
                p.height * 1000.0
            };
            format!("Edge {} selected: {:.0} mm", e + 1, mm)
        });

    ui! {
        <div id="viewport-2d" class="viewport-container">
            <div class="viewport-header">
                <span class="viewport-title">"2D REST PATTERN"</span>
                <span class="viewport-sub">{edge_hint}</span>
            </div>
            <div class="canvas-area">
                <svg viewBox="0 0 400 400" class="svg-canvas">
                    {svg_nodes}
                </svg>
            </div>
        </div>
    }
}

fn build_right_dock(state: &CssEditorState) -> Node {
    let (tab_p, tab_o, tab_f, tab_s) = match state.active_tab {
        EditorTab::Properties => ("dock-tab active", "dock-tab", "dock-tab", "dock-tab"),
        EditorTab::Objects => ("dock-tab", "dock-tab active", "dock-tab", "dock-tab"),
        EditorTab::Fabric => ("dock-tab", "dock-tab", "dock-tab active", "dock-tab"),
        EditorTab::Simulation => ("dock-tab", "dock-tab", "dock-tab", "dock-tab active"),
    };

    let content = match state.active_tab {
        EditorTab::Properties => build_tab_properties(state),
        EditorTab::Objects => build_tab_objects(state),
        EditorTab::Fabric => build_tab_fabric(state),
        EditorTab::Simulation => build_tab_simulation(state),
    };

    ui! {
        <aside id="right-dock" class="right-dock">
            <div class="dock-tabs">
                <button class={tab_p} onclick={on_tab_properties}>"Properties"</button>
                <button class={tab_o} onclick={on_tab_objects}>"Objects"</button>
                <button class={tab_f} onclick={on_tab_fabric}>"Fabric"</button>
                <button class={tab_s} onclick={on_tab_simulation}>"Simulation"</button>
            </div>
            <div class="dock-content">
                {content}
                {build_mesh_preparation_accordion(state)}
            </div>
        </aside>
    }
}

fn build_tab_properties(state: &CssEditorState) -> Node {
    let p = &state.active.pattern;
    let width_str = format!("{:.0} mm", p.width * 1000.0);
    let height_str = format!("{:.0} mm", p.height * 1000.0);
    let res_str = format!("{} × {} cells", p.nx, p.ny);

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Pattern Dimensions"</h3>
            <div class="prop-row">
                <span class="prop-label">"Width"</span>
                <span class="prop-value">{width_str}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Height"</span>
                <span class="prop-value">{height_str}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Resolution"</span>
                <span class="prop-value">{res_str}</span>
            </div>
        </div>
    }
}

fn build_tab_objects(state: &CssEditorState) -> Node {
    let verts = state.mesh.uv.len();
    let tris = state.mesh.triangles.len();
    let pins = state.mesh.pins.iter().filter(|p| p.is_some()).count();

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Reference Mesh Objects"</h3>
            <div class="prop-row">
                <span class="prop-label">"Vertices"</span>
                <span class="prop-value">{format!("{verts}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Triangles"</span>
                <span class="prop-value">{format!("{tris}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Fixed Pins"</span>
                <span class="prop-value">{format!("{pins}")}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Selected Edge"</span>
                <span class="prop-value">
                    {state.selected_edge.map_or("None".to_string(), |e| format!("Edge {}", e + 1))}
                </span>
            </div>
            <button class="action-btn" onclick={on_edge_select_next}>"Select Next Boundary Edge"</button>
        </div>
    }
}

fn build_tab_fabric(state: &CssEditorState) -> Node {
    let mat = &state.active.material;
    let density_str = format!(
        "{:.3} kg/m² ({:.0} g/m²)",
        mat.density,
        mat.density * 1000.0
    );
    let stretch_str = format!("Warp {:.0} N/m, Weft {:.0} N/m", mat.warp, mat.weft);
    let grain_str = format!("{:.1}°", mat.grain_degrees);

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Fabric Properties"</h3>
            <div class="prop-row">
                <span class="prop-label">"Density"</span>
                <span class="prop-value">{density_str}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Stretch Stiffness"</span>
                <span class="prop-value">{stretch_str}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Warp Bending"</span>
                <span class="prop-value">{format!("{:.2e} N·m", mat.bend_warp)}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Weft Bending"</span>
                <span class="prop-value">{format!("{:.2e} N·m", mat.bend_weft)}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Grain Angle"</span>
                <span class="prop-value">{grain_str}</span>
            </div>
        </div>
    }
}

fn build_tab_simulation(state: &CssEditorState) -> Node {
    let s = &state.active.solver;
    let dt_str = format!("{:.3} s ({:.1} Hz)", s.dt, 1.0 / s.dt);
    let tol_str = format!("{:.1e}", s.absolute_tolerance);
    let last_pen = state.metrics.as_ref().map_or("0.0 mm".to_string(), |m| {
        format!("{:.3} mm", m.max_vertex_penetration_m * 1000.0)
    });
    let last_res = state
        .metrics
        .as_ref()
        .map_or("N/A".to_string(), |m| format!("{:.1e}", m.residual_rms_n));

    ui! {
        <div class="tab-panel">
            <h3 class="panel-heading">"Solver Configuration"</h3>
            <div class="prop-row">
                <span class="prop-label">"Time step dt"</span>
                <span class="prop-value">{dt_str}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Newton iterations"</span>
                <span class="prop-value">{format!("{}", s.newton_iterations)}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"PCG iterations"</span>
                <span class="prop-value">{format!("{}", s.pcg_iterations)}</span>
            </div>
            <div class="prop-row">
                <span class="prop-label">"Tolerance"</span>
                <span class="prop-value">{tol_str}</span>
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
    }
}

fn build_mesh_preparation_accordion(state: &CssEditorState) -> Node {
    let err_node = if let Some(err) = &state.mesh_prep_error {
        ui! { <p class="prep-error">{err.clone()}</p> }
    } else {
        ui! { <span></span> }
    };

    ui! {
        <div class="accordion-section">
            <h4 class="accordion-title">"Physics-inspired mesh preparation"</h4>
            <p class="accordion-sub">
                "Build before simulating. Bending and grain come from Fabric settings; E is an explicit sizing estimate."
            </p>
            <div class="prep-inputs">
                <div class="prop-row">
                    <span class="prop-label">"Warp E (Pa)"</span>
                    <span class="prop-value">{state.mesh_prep_warp_e.clone()}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Weft E (Pa)"</span>
                    <span class="prop-value">{state.mesh_prep_weft_e.clone()}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Boundary ratio"</span>
                    <span class="prop-value">{state.mesh_prep_ratio.clone()}</span>
                </div>
                <div class="prop-row">
                    <span class="prop-label">"Transition (mm)"</span>
                    <span class="prop-value">{state.mesh_prep_transition.clone()}</span>
                </div>
            </div>
            {err_node}
            <div class="prep-buttons">
                <button id="remesh-reset-btn" class="action-btn primary" onclick={on_remesh_reset}>
                    "Remesh and reset"
                </button>
                <button id="uniform-reset-btn" class="action-btn secondary" onclick={on_uniform_reset}>
                    "Use uniform grid and reset"
                </button>
            </div>
        </div>
    }
}

fn build_status_bar(state: &CssEditorState) -> Node {
    let max_pen_mm = state
        .metrics
        .as_ref()
        .map_or(0.0, |m| m.max_vertex_penetration_m * 1000.0);
    let residual = state.metrics.as_ref().map_or(0.0, |m| m.residual_rms_n);

    let step_info = if state.gpu_sim {
        format!(
            "Step: {} | computation: {:.1} ms | Residual: unavailable | Penetration audit: unavailable",
            state.step_count, state.step_ms
        )
    } else {
        format!(
            "Step: {} | dt: {:.1} ms | Residual: {:.1e} | Max Pen: {:.3} mm",
            state.step_count, state.step_ms, residual, max_pen_mm
        )
    };
    let mesh_info = format!(
        "Verts: {} | Tris: {}",
        state.mesh.uv.len(),
        state.mesh.triangles.len()
    );

    ui! {
        <footer class="status-bar">
            <div class="status-left">
                <span class="status-indicator">"●"</span>
                <span class="status-message">{state.status_message.clone()}</span>
            </div>
            <div class="status-right">
                <span class="status-step-info">{step_info}</span>
                <span class="status-mesh-info">{mesh_info}</span>
            </div>
        </footer>
    }
}

fn build_command_palette_overlay(state: &CssEditorState) -> Node {
    if !state.command_palette_open {
        return ui! { <div id="command-palette-container"></div> };
    }

    ui! {
        <div id="command-palette-container" class="command-palette-backdrop">
            <div class="command-palette-modal">
                <div class="palette-header">
                    <span class="palette-title">"Command Palette"</span>
                    <button class="palette-close-btn" onclick={on_palette_toggle}>"✕"</button>
                </div>
                <div class="palette-items">
                    <button class="palette-item" onclick={on_play_pause}>"▶ Toggle Simulation (Space)"</button>
                    <button class="palette-item" onclick={on_step}>"⏭ Step Simulation"</button>
                    <button class="palette-item" onclick={on_reset}>"↺ Reset Simulation"</button>
                    <button class="palette-item" onclick={on_save}>"💾 Save Project (Ctrl+S)"</button>
                    <button class="palette-item" onclick={on_export_obj}>"📦 Export OBJ"</button>
                    <button class="palette-item" onclick={on_scene_sphere}>"🌐 Scene: Sphere Benchmark"</button>
                    <button class="palette-item" onclick={on_scene_hang}>"📌 Scene: Hanging Pins"</button>
                    <button class="palette-item" onclick={on_remesh_reset}>"⚡ Remesh and Reset"</button>
                    <button class="palette-item" onclick={on_uniform_reset}>"▦ Use Uniform Grid and Reset"</button>
                    <button class="palette-item" onclick={on_theme_toggle}>"🎨 Toggle Dark/Light Theme"</button>
                </div>
            </div>
        </div>
    }
}

// ============================================================================
// 2D Pattern Board SVG
// ============================================================================

fn render_2d_pattern_svg(state: &CssEditorState) -> Vec<Node> {
    let mut nodes = Vec::new();

    // Board background
    nodes.push(ui! {
        <rect width="400" height="400" fill="#181b22" />
    });

    // Drafting grid lines
    let grid_step = 40.0_f32;
    let mut gx = grid_step;
    while gx < 400.0 {
        nodes.push(ui! {
            <polyline points={format!("{gx:.1},0 {gx:.1},400")} stroke="#262a34" stroke-width="0.8" />
        });
        gx += grid_step;
    }
    let mut gy = grid_step;
    while gy < 400.0 {
        nodes.push(ui! {
            <polyline points={format!("0,{gy:.1} 400,{gy:.1}")} stroke="#262a34" stroke-width="0.8" />
        });
        gy += grid_step;
    }

    let p = &state.active.pattern;
    let pw = p.width as f32;
    let ph = p.height as f32;
    let max_dim = pw.max(ph).max(0.01);
    let scale = 280.0 / max_dim;
    let ox = 200.0 - pw * scale * 0.5;
    let oy = 200.0 - ph * scale * 0.5;

    let uv_to_screen =
        |u: glam::DVec2| -> (f32, f32) { (ox + u.x as f32 * scale, oy + u.y as f32 * scale) };

    // Render 2D mesh triangles
    for t in &state.mesh.triangles {
        let (x0, y0) = uv_to_screen(state.mesh.uv[t.vertices[0]]);
        let (x1, y1) = uv_to_screen(state.mesh.uv[t.vertices[1]]);
        let (x2, y2) = uv_to_screen(state.mesh.uv[t.vertices[2]]);
        let points = format!("{x0:.1},{y0:.1} {x1:.1},{y1:.1} {x2:.1},{y2:.1}");

        nodes.push(ui! {
            <polygon points={points} fill="rgba(148, 163, 184, 0.12)" stroke="#64748b" stroke-width="0.6" />
        });
    }

    // Four boundary edges
    let corners = [
        (ox, oy),
        (ox + pw * scale, oy),
        (ox + pw * scale, oy + ph * scale),
        (ox, oy + ph * scale),
    ];

    for i in 0..4 {
        let (x_start, y_start) = corners[i];
        let (x_end, y_end) = corners[(i + 1) % 4];
        let is_selected = state.selected_edge == Some(i);
        let stroke_color = if is_selected { "#f59e0b" } else { "#f0f3f8" };
        let stroke_w = if is_selected { "3.0" } else { "1.5" };

        let pts = format!("{x_start:.1},{y_start:.1} {x_end:.1},{y_end:.1}");
        nodes.push(ui! {
            <polyline points={pts} stroke={stroke_color} stroke-width={stroke_w} />
        });
    }

    // Warp grainline arrow
    let grain_rad = (state.active.material.grain_degrees as f32).to_radians();
    let cx = 200.0_f32;
    let cy = 200.0_f32;
    let d = 50.0_f32;
    let ax = cx + grain_rad.cos() * d;
    let ay = cy + grain_rad.sin() * d;
    let arrow_pts = format!("{cx:.1},{cy:.1} {ax:.1},{ay:.1}");
    nodes.push(ui! {
        <polyline points={arrow_pts} stroke="#3b82f6" stroke-width="2.2" />
    });

    // Fixed pins on 2D board
    for (i, pin) in state.mesh.pins.iter().enumerate() {
        if pin.is_some() {
            let (px, py) = uv_to_screen(state.mesh.uv[i]);
            nodes.push(ui! {
                <circle cx={format!("{px:.1}")} cy={format!("{py:.1}")} r="4.0"
                    fill="#ef4444" stroke="#ffffff" stroke-width="1.2" />
            });
        }
    }

    nodes
}

// ============================================================================
// App Constructor and Native Window Runner
// ============================================================================

#[allow(clippy::type_complexity)]
pub fn create_editor_app<'a>(
    mut state: CssEditorState,
    stylesheet: &'a Stylesheet,
) -> App<
    'a,
    CssEditorState,
    fn(&mut CssEditorState, FrameInfo) -> Invalidation,
    fn(&CssEditorState) -> Node,
    Invalidation,
> {
    let preview_path = state.output_dir.join("editor_preview.usda");
    if let Some(parent) = preview_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let usda =
        marvelous_cloth::usd_scene::mesh_stage_usda(&state.mesh, &state.state).unwrap_or_default();
    let usda = if let Some(sphere) = &state.active.sphere {
        marvelous_cloth::usd_scene::append_viewport_surface(
            usda,
            &marvelous_cloth::usd_scene::viewport_sphere(sphere),
        )
    } else {
        usda
    };
    let _ = std::fs::write(&preview_path, usda);

    let usd_scene = cssimpler::renderer::gpu::usd::UsdScene::open(&preview_path)
        .expect("load generated USD viewport scene");
    state.usd_scene = Some(usd_scene.clone());

    let app = App::new(
        state,
        stylesheet,
        update_editor as fn(&mut CssEditorState, FrameInfo) -> Invalidation,
        build_editor_ui as fn(&CssEditorState) -> Node,
    )
    .with_event_handler(handle_editor_event)
    .with_render_mode(RenderMode::OnInvalidation)
    .with_continuous_updates(true);

    #[cfg(feature = "desktop")]
    let app = app.with_usd_viewport(
        "viewport-3d-scene",
        usd_scene,
        cssimpler::renderer::gpu::usd::UsdCameraMode::Orbit,
    );

    app
}

pub fn run(project: Project, output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    run_with_cpu(project, output, false)
}

pub fn run_with_cpu(
    project: Project,
    output: PathBuf,
    cpu: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let stylesheet = Box::leak(Box::new(load_studio_stylesheet()?));
    let gpu = if cpu {
        None
    } else {
        let (device, queue) = marvelous_cloth::gpu_avbd::request_device()?;
        Some(marvelous_cloth::gpu_avbd::ProjectAvbd::new(device, queue))
    };
    let state = CssEditorState::new_with_backend(project, output, gpu);
    let app = create_editor_app(state, stylesheet);
    let config = WindowConfig::new("MARVELOUS CLOTH | FEM Workshop", 1600, 980);
    app.run(config)?;
    Ok(())
}

// ============================================================================
// Unit Tests (Headless Frame & RGBA8 verification)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    #[ignore = "requires explicit coordination and a hardware GPU"]
    fn wgsl_project_worker_advances_without_render_frames() {
        let (device, queue) = marvelous_cloth::gpu_avbd::request_device().unwrap();
        let mut project = Project {
            sphere: None,
            ..Default::default()
        };
        project.pattern.nx = 2;
        project.pattern.ny = 2;
        let dt = project.solver.dt;
        let mut state = CssEditorState::new_with_backend(
            project,
            PathBuf::from("target/test_gpu_editor"),
            Some(marvelous_cloth::gpu_avbd::ProjectAvbd::new(device, queue)),
        );
        state.send_control_state(true);
        let deadline = Instant::now() + Duration::from_secs(5);
        let done = loop {
            if let Some(done) = state.worker_latest.lock().unwrap().take() {
                break done;
            }
            assert!(
                Instant::now() < deadline,
                "worker did not produce a step without rendering"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(done.generation, state.generation);
        let (accepted, metrics) = done.result.unwrap();
        assert!(matches!(&metrics, EditorMetrics::Avbd(_)));
        state.accept_step(accepted, metrics);
        assert_eq!(state.state.time, dt);
        assert_eq!(state.step_count, 1);
        assert!(state.metrics.is_none());
        assert!(state.status_message.contains("WGSL AVBD / IPC"));
    }

    #[test]
    fn test_css_editor_headless_frame_and_rendering() {
        let stylesheet = load_studio_stylesheet().expect("load studio stylesheet");
        let project = Project::default();
        let state = CssEditorState::new(project, PathBuf::from("target/test_editor_out"));
        let mut app = create_editor_app(state, &stylesheet);
        app.set_viewport(cssimpler::renderer::ViewportSize::new(1600, 980));

        // Advance one frame headlessly
        let scene = app.frame(FrameInfo {
            delta: Duration::from_millis(16),
            frame_index: 0,
        });
        assert!(!scene.is_empty(), "scene nodes should not be empty");

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
        // Verify key layout containers exist
        assert!(app.find_element_bounds_by_id("#viewport-3d").is_some());
        let usd_bounds = app
            .find_element_bounds_by_id("#viewport-3d-scene")
            .expect("USD viewport exists");
        assert!(
            usd_bounds.width > 100.0 && usd_bounds.height > 100.0,
            "USD viewport must have drawable area: {usd_bounds:?}"
        );
        assert!(app.find_element_bounds_by_id("#viewport-2d").is_some());
        assert!(app.find_element_bounds_by_id("#right-dock").is_some());

        // Test RGBA8 rendering headlessly
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
            "rendered buffer should contain painted UI pixels"
        );

        let read_points = |state: &CssEditorState| {
            state
                .usd_scene
                .as_ref()
                .unwrap()
                .stage()
                .attribute("/World/Garment/cloth_mesh.points")
                .unwrap()
                .get::<Vec<cssimpler::renderer::gpu::openusd::gf::Vec3f>>()
                .unwrap()
                .unwrap()
        };
        let initial_points = read_points(app.state());
        // Test Step
        assert_eq!(app.state().step_count, 0);
        app.state_mut().step();
        assert_eq!(app.state().step_count, 1);
        assert!(app.state().metrics.is_some());
        assert_ne!(
            read_points(app.state()),
            initial_points,
            "accepted steps must update USD points"
        );

        // Test Reset
        app.state_mut().reset();
        assert_eq!(app.state().step_count, 0);
        assert!(app.state().metrics.is_none());
        assert_eq!(
            read_points(app.state()),
            initial_points,
            "reset restores USD points"
        );

        // Test Tab Switching
        assert_eq!(app.state().active_tab, EditorTab::Properties);
        app.state_mut().active_tab = EditorTab::Fabric;
        assert_eq!(app.state().active_tab, EditorTab::Fabric);

        // Test Mesh Preparation: Remesh and Reset
        app.state_mut().remesh_and_reset();
        assert!(app.state().mesh_prep_error.is_none());
        assert!(app.state().active.physics_sizing.is_some());

        // Test Mesh Preparation: Uniform Reset
        app.state_mut().uniform_grid_and_reset();
        assert!(app.state().active.physics_sizing.is_none());
    }
}

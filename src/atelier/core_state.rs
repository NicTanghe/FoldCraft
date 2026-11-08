//! UI-agnostic Atelier workspace core state, command registry facade, and worker boundaries.
//!
//! Decouples project data, compiled scene, simulation settings, worker channels,
//! selection state, tool modes, and draft guards from `eframe::App`.
#![allow(dead_code)]
use super::{
    commands::{self, CapabilityContext, CommandId, UnavailableReason},
    worker::{self, FinalDiagnosticsRequest, Latest, RepaintSignal, Work},
    workspace_profiles,
};
use marvelous_cloth::{
    fitting::{FittingProject, FittingScene},
    garment::{export, presets::GarmentKind, seam::BoundaryRef, CompiledGarment},
    model::State,
    vbd::{Grab, VbdMetrics, VbdSettings},
};
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
};

pub use workspace_profiles::Store as WorkspaceProfileStore;

/// Active authoring tool modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tool {
    Grab,
    #[default]
    Edit,
    Place,
    Draw,
    Rectangle,
    InternalLine,
    Measure,
    Sew,
    Inspect,
    Ellipse,
    Sector,
}

/// 2D / 3D selection state across pieces, seams, edges, points, and trims.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionState {
    pub primary_panel: Option<String>,
    pub panels: Vec<String>,
    pub seam: Option<String>,
    pub edge: Option<BoundaryRef>,
    pub point: Option<(String, usize)>,
    pub trim: Option<String>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.primary_panel.is_none()
            && self.panels.is_empty()
            && self.seam.is_none()
            && self.edge.is_none()
            && self.point.is_none()
            && self.trim.is_none()
    }

    pub fn select_panel(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.primary_panel = Some(id.clone());
        if !self.panels.contains(&id) {
            self.panels.push(id);
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Piece / pattern panel summary.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceSummary {
    pub id: String,
    pub label: String,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub boundary_count: usize,
    pub measurement_count: usize,
    pub has_polygon: bool,
}

/// Seam summary connecting boundary edges.
#[derive(Clone, Debug, PartialEq)]
pub struct SeamSummary {
    pub id: String,
    pub left_panel: String,
    pub left_boundary: String,
    pub right_panel: String,
    pub right_boundary: String,
}

/// Material / fabric summary.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSummary {
    pub id: String,
    pub name: String,
    pub density_kg_m2: f64,
    pub stretch_stiffness: f64,
    pub bend_stiffness: f64,
    pub has_texture: bool,
}

/// Trim summary.
#[derive(Clone, Debug, PartialEq)]
pub struct TrimSummary {
    pub id: String,
    pub panel_id: String,
    pub kind_name: String,
}

/// Rest / world pin summary.
#[derive(Clone, Debug, PartialEq)]
pub struct PinSummary {
    pub panel_id: String,
    pub coordinates: [f64; 2],
    pub world_target: [f64; 3],
}

/// Pattern measurement summary.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementSummary {
    pub id: String,
    pub label: String,
    pub panel_id: String,
    pub length_mm: f64,
}

/// Reject commands that reload the scene and discard an uncommitted trim draft.
pub fn check_trim_draft_for_command(
    id: CommandId,
    trim_pending: bool,
) -> Result<(), &'static str> {
    if trim_pending {
        return match id {
            CommandId::LoadTshirt | CommandId::LoadDress => {
                Err("Apply or cancel the trim draft before loading an example")
            }
            CommandId::ResetSimulation => {
                Err("Apply or cancel the trim draft before resetting the simulation")
            }
            _ => Ok(()),
        };
    }
    Ok(())
}

/// Reject commands that reload the scene and discard an uncommitted measurement draft.
pub fn check_measurement_draft_for_command(
    id: CommandId,
    measurement_pending: bool,
) -> Result<(), &'static str> {
    if measurement_pending {
        return match id {
            CommandId::LoadTshirt | CommandId::LoadDress => {
                Err("Apply or cancel the measurement draft before loading an example")
            }
            CommandId::ResetSimulation => {
                Err("Apply or cancel the measurement draft before resetting the simulation")
            }
            _ => Ok(()),
        };
    }
    Ok(())
}

/// Compute piece summaries from project template and compiled scene garment.
pub fn piece_summaries(project: &FittingProject, scene: &FittingScene) -> Vec<PieceSummary> {
    project
        .template
        .panels
        .iter()
        .map(|panel| {
            let garment_panel = scene.garment.panels.iter().find(|p| p.id == panel.id);
            let vertex_count = garment_panel.map_or(0, |p| p.vertices.len());
            let triangle_count = garment_panel.map_or(0, |p| p.triangles.len());
            let boundary_count = panel.polygon.as_ref().map_or(0, |p| p.edges.len());
            let measurement_count = panel.polygon.as_ref().map_or(0, |p| p.measurements.len());
            let has_polygon = panel.polygon.is_some();
            PieceSummary {
                id: panel.id.clone(),
                label: panel.name.clone(),
                vertex_count,
                triangle_count,
                boundary_count,
                measurement_count,
                has_polygon,
            }
        })
        .collect()
}

/// Compute seam summaries from project template.
pub fn seam_summaries(project: &FittingProject) -> Vec<SeamSummary> {
    project
        .template
        .seams
        .iter()
        .map(|seam| SeamSummary {
            id: seam.id.clone(),
            left_panel: seam.a.panel.clone(),
            left_boundary: seam.a.edge.as_deref().unwrap_or("boundary").to_owned(),
            right_panel: seam.b.panel.clone(),
            right_boundary: seam.b.edge.as_deref().unwrap_or("boundary").to_owned(),
        })
        .collect()
}

/// Compute material summaries from project template.
pub fn material_summaries(project: &FittingProject) -> Vec<MaterialSummary> {
    project
        .template
        .fabrics
        .iter()
        .map(|fabric| MaterialSummary {
            id: fabric.id.clone(),
            name: fabric.name.clone(),
            density_kg_m2: fabric.properties.density,
            stretch_stiffness: fabric.properties.weft,
            bend_stiffness: fabric.properties.bend_weft,
            has_texture: project
                .template
                .fabric_appearance
                .contains_key(&fabric.id),
        })
        .collect()
}

/// Compute trim summaries from project template.
pub fn trim_summaries(project: &FittingProject) -> Vec<TrimSummary> {
    project
        .template
        .trims
        .iter()
        .map(|trim| {
            let kind_name = match &trim.kind {
                marvelous_cloth::garment::trims::TrimKind::Button { .. } => "button",
                marvelous_cloth::garment::trims::TrimKind::Buttonhole { .. } => "buttonhole",
                marvelous_cloth::garment::trims::TrimKind::Zipper { .. } => "zipper",
                marvelous_cloth::garment::trims::TrimKind::Topstitch { .. } => "topstitch",
            };
            TrimSummary {
                id: trim.id.clone(),
                panel_id: trim.panel_id.clone(),
                kind_name: kind_name.to_owned(),
            }
        })
        .collect()
}

/// Compute pin summaries from project template.
pub fn pin_summaries(project: &FittingProject) -> Vec<PinSummary> {
    project
        .template
        .pin_targets
        .iter()
        .map(|pin| PinSummary {
            panel_id: pin.panel.clone(),
            coordinates: [pin.uv.x, pin.uv.y],
            world_target: [pin.world.x, pin.world.y, pin.world.z],
        })
        .collect()
}

/// Compute measurement summaries from polygon rest geometry in mm.
pub fn measurement_summaries(project: &FittingProject) -> Vec<MeasurementSummary> {
    let mut summaries = Vec::new();
    for panel in &project.template.panels {
        if let Some(poly) = &panel.polygon {
            for m in &poly.measurements {
                let length_mm = poly.measurement_length(m).unwrap_or(0.0) * 1000.0;
                summaries.push(MeasurementSummary {
                    id: m.id.clone(),
                    label: m.label.clone(),
                    panel_id: panel.id.clone(),
                    length_mm,
                });
            }
        }
    }
    summaries
}

/// Write fitting project artifacts to disk.
pub fn export_session(
    project: &FittingProject,
    scene: &FittingScene,
    settings: &VbdSettings,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output)?;
    export::write_artifacts(&project.template, &scene.garment, &scene.state, output)?;
    let mut p = project.clone();
    p.state = Some(scene.state.clone());
    p.simulation = Some(settings.clone());
    p.save(&output.join("fitting-project.json"))?;
    Ok(())
}

/// Clean, UI-agnostic workspace core state facade.
pub struct AtelierCore {
    pub project: FittingProject,
    pub scene: FittingScene,
    pub settings: VbdSettings,
    pub output: PathBuf,
    pub generation: u64,
    pub send: Option<mpsc::Sender<Work>>,
    pub receive: Latest,
    pub signal: RepaintSignal,
    pub playing: bool,
    pub pending: bool,
    pub gpu: bool,
    pub metrics: Option<VbdMetrics>,
    pub status: String,
    pub simulation_error: Option<String>,
    pub grab: Option<Grab>,
    pub tool: Tool,
    pub selection: SelectionState,
    pub profiles: WorkspaceProfileStore,
    pub mesh_mm: f64,
    pub anisotropy: f64,
    pub mesh_axis: f64,
    pub pending_trim_draft: bool,
    pub pending_measurement_draft: bool,
    pub _held_rx: Option<Arc<Mutex<mpsc::Receiver<Work>>>>,
}

impl AtelierCore {
    pub fn new(
        project: FittingProject,
        scene: FittingScene,
        output: PathBuf,
        send: Option<mpsc::Sender<Work>>,
        receive: Latest,
        signal: RepaintSignal,
    ) -> Self {
        let m = project
            .template
            .panels
            .iter()
            .find_map(|p| p.polygon.as_ref())
            .map(|p| p.meshing_settings())
            .unwrap_or_default();
        let primary_panel = project.template.panels.first().map(|p| p.id.clone());
        let panels = primary_panel.clone().into_iter().collect();
        let simulation = project.simulation.clone().unwrap_or_default();
        Self {
            project,
            scene,
            settings: simulation,
            output,
            generation: 0,
            send,
            receive,
            signal,
            playing: false,
            pending: false,
            gpu: false,
            metrics: None,
            status: "Ready · Space to start the simulation".into(),
            simulation_error: None,
            grab: None,
            tool: Tool::Edit,
            selection: SelectionState {
                primary_panel,
                panels,
                seam: None,
                edge: None,
                point: None,
                trim: None,
            },
            profiles: WorkspaceProfileStore::default(),
            mesh_mm: m.target_edge_m * 1000.0,
            anisotropy: m.anisotropy,
            mesh_axis: m.axis_degrees,
            pending_trim_draft: false,
            pending_measurement_draft: false,
            _held_rx: None,
        }
    }

    /// Construct a headless core with a retained mock worker channel for testing.
    pub fn headless(project: FittingProject, scene: FittingScene) -> Self {
        let (send, rx) = mpsc::channel();
        let receive = Arc::new(Mutex::new(None));
        let mut core = Self::new(
            project,
            scene,
            PathBuf::from("output"),
            Some(send),
            receive,
            RepaintSignal::noop(),
        );
        core._held_rx = Some(Arc::new(Mutex::new(rx)));
        core
    }

    /// Construct a headless core compiled from a `FittingProject`.
    pub fn from_project(project: FittingProject) -> Result<Self, String> {
        let scene = project.compile().map_err(|e| e.to_string())?;
        Ok(Self::headless(project, scene))
    }

    #[inline]
    pub fn project(&self) -> &FittingProject {
        &self.project
    }

    #[inline]
    pub fn project_mut(&mut self) -> &mut FittingProject {
        &mut self.project
    }

    #[inline]
    pub fn compiled_garment(&self) -> &CompiledGarment {
        &self.scene.garment
    }

    #[inline]
    pub fn state(&self) -> &State {
        &self.scene.state
    }

    #[inline]
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.scene.state
    }

    #[inline]
    pub fn settings(&self) -> &VbdSettings {
        &self.settings
    }

    #[inline]
    pub fn settings_mut(&mut self) -> &mut VbdSettings {
        &mut self.settings
    }

    #[inline]
    pub fn active_tool(&self) -> Tool {
        self.tool
    }

    #[inline]
    pub fn set_active_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    #[inline]
    pub fn selection(&self) -> &SelectionState {
        &self.selection
    }

    #[inline]
    pub fn selection_mut(&mut self) -> &mut SelectionState {
        &mut self.selection
    }

    #[inline]
    pub fn profiles(&self) -> &WorkspaceProfileStore {
        &self.profiles
    }

    #[inline]
    pub fn profiles_mut(&mut self) -> &mut WorkspaceProfileStore {
        &mut self.profiles
    }

    #[inline]
    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn set_playing(&mut self, playing: bool) -> Result<(), String> {
        self.playing = playing;
        if self.send.is_some() {
            self.submit_work(false)?;
        }
        Ok(())
    }

    #[inline]
    pub fn has_pending_trim_draft(&self) -> bool {
        self.pending_trim_draft
    }

    #[inline]
    pub fn set_pending_trim_draft(&mut self, pending: bool) {
        self.pending_trim_draft = pending;
    }

    #[inline]
    pub fn has_pending_measurement_draft(&self) -> bool {
        self.pending_measurement_draft
    }

    #[inline]
    pub fn set_pending_measurement_draft(&mut self, pending: bool) {
        self.pending_measurement_draft = pending;
    }

    pub fn capability_context(&self) -> CapabilityContext {
        let selected_panel = self.selection.primary_panel.as_ref().is_some_and(|id| {
            self.project
                .template
                .panels
                .iter()
                .any(|panel| &panel.id == id)
        });
        CapabilityContext {
            document_loaded: !self.project.template.panels.is_empty(),
            selected_panel,
            worker_ready: self.send.is_some(),
            filesystem_ready: true,
            playing: self.playing,
            pending: self.pending,
        }
    }

    pub fn command_availability(&self, id: CommandId) -> Result<(), UnavailableReason> {
        commands::availability(id, self.capability_context())
    }

    pub fn execute_command(&mut self, id: CommandId) -> Result<(), String> {
        commands::availability(id, self.capability_context())
            .map_err(|reason| reason.message().to_owned())?;
        check_trim_draft_for_command(id, self.has_pending_trim_draft())
            .map_err(str::to_owned)?;
        check_measurement_draft_for_command(id, self.has_pending_measurement_draft())
            .map_err(str::to_owned)?;
        match id {
            CommandId::ToggleSimulation => {
                let next = !self.playing;
                self.set_playing(next)?;
            }
            CommandId::StepSimulation => {
                self.step_once()?;
            }
            CommandId::ResetSimulation => {
                self.reset_drape()?;
            }
            CommandId::ExportSession => {
                self.export_session()?;
            }
            CommandId::SplitView
            | CommandId::GarmentView
            | CommandId::PatternView
            | CommandId::ToggleInspector
            | CommandId::ResetLayout
            | CommandId::OpenPreferences => {
                // UI preference commands are acknowledged; the UI shell reflects them.
            }
            CommandId::LoadTshirt => {
                self.load_preset(GarmentKind::Tshirt)?;
            }
            CommandId::LoadDress => {
                self.load_preset(GarmentKind::Dress)?;
            }
            CommandId::ToolEdit => self.set_active_tool(Tool::Edit),
            CommandId::ToolDraw | CommandId::ToolFreehand => self.set_active_tool(Tool::Draw),
            CommandId::ToolRectangle => self.set_active_tool(Tool::Rectangle),
            CommandId::ToolEllipse => self.set_active_tool(Tool::Ellipse),
            CommandId::ToolSector => self.set_active_tool(Tool::Sector),
            CommandId::ToolInternalLine => self.set_active_tool(Tool::InternalLine),
            CommandId::ToolSew => self.set_active_tool(Tool::Sew),
            CommandId::ToolMove | CommandId::ToolRotate => self.set_active_tool(Tool::Place),
            CommandId::ToolGrab => self.set_active_tool(Tool::Grab),
            CommandId::ToolMeasure => self.set_active_tool(Tool::Measure),
            CommandId::ToolInspect => self.set_active_tool(Tool::Inspect),
        }
        Ok(())
    }

    pub fn submit_work(&mut self, once: bool) -> Result<(), String> {
        let sender = self
            .send
            .as_ref()
            .ok_or_else(|| "The simulation worker is unavailable".to_owned())?;
        let scene = FittingScene {
            garment: self.scene.garment.clone(),
            collider: self.scene.collider.clone(),
            state: self.scene.state.clone(),
            quasistatic: false,
            next_dt: self.settings.dt,
        };
        sender
            .send(Work {
                generation: self.generation,
                playing: self.playing,
                once,
                scene,
                settings: self.settings.clone(),
                grab: self.grab.clone(),
                gpu: self.gpu,
                signal: self.signal.clone(),
            })
            .map_err(|_| "The simulation worker is unavailable".to_owned())?;
        self.pending = self.playing || once;
        Ok(())
    }

    pub fn step_once(&mut self) -> Result<(), String> {
        self.submit_work(true)
    }

    pub fn load_project(&mut self, p: FittingProject) -> Result<(), String> {
        if self.has_pending_trim_draft() {
            return Err("Apply or cancel the trim draft before loading another document".into());
        }
        if self.has_pending_measurement_draft() {
            return Err("Apply or cancel the measurement draft before loading another document".into());
        }
        let scene = p.compile().map_err(|e| e.to_string())?;
        if let Some(poly) = p.template.panels.iter().find_map(|panel| panel.polygon.as_ref()) {
            let m = poly.meshing_settings();
            self.mesh_mm = m.target_edge_m * 1000.0;
            self.anisotropy = m.anisotropy;
            self.mesh_axis = m.axis_degrees;
        }
        self.selection.primary_panel = p.template.panels.first().map(|panel| panel.id.clone());
        self.selection.panels = self.selection.primary_panel.clone().into_iter().collect();
        self.scene = scene;
        self.project = p;
        self.generation += 1;
        self.playing = false;
        self.pending = false;
        self.metrics = None;
        self.grab = None;
        self.status = "New arrangement · run to assemble the seams".into();
        if self.send.is_some() {
            let _ = self.submit_work(false);
        }
        Ok(())
    }

    pub fn load_preset(&mut self, kind: GarmentKind) -> Result<(), String> {
        let project = FittingProject::basic(kind, 2, self.project.avatar_path.clone());
        self.load_project(project)
    }

    pub fn reset_drape(&mut self) -> Result<(), String> {
        if self.has_pending_trim_draft() {
            return Err("Apply or cancel the trim draft before resetting the simulation".into());
        }
        if self.has_pending_measurement_draft() {
            return Err("Apply or cancel the measurement draft before resetting the simulation".into());
        }
        let mut p = self.project.clone();
        p.state = None;
        p.mesh_signature = None;
        p.prearrange = false;
        self.load_project(p)
    }

    pub fn export_session(&mut self) -> Result<(), String> {
        export_session(&self.project, &self.scene, &self.settings, &self.output)
            .map_err(|e| e.to_string())
    }

    pub fn remesh(&mut self, target_edge_mm: f64) -> Result<(), String> {
        let mut p = self.project.clone();
        p.state = None;
        p.mesh_signature = None;
        p.prearrange = false;
        for panel in &mut p.template.panels {
            if let Some(poly) = &mut panel.polygon {
                let mut m = poly.meshing_settings();
                m.target_edge_m = target_edge_mm * 0.001;
                m.anisotropy = self.anisotropy;
                m.axis_degrees = self.mesh_axis;
                poly.meshing = Some(m);
            }
        }
        self.load_project(p)
    }

    pub fn request_final_diagnostics(&self) -> FinalDiagnosticsRequest {
        worker::request_final_diagnostics(self.generation)
    }

    pub fn poll_worker(&mut self) -> Option<Result<(), String>> {
        let done = self.receive.lock().ok()?.take()?;
        if done.generation != self.generation {
            return None;
        }
        self.pending = false;
        match done.result {
            Ok((state, metrics)) => {
                self.simulation_error = None;
                self.scene.state = state;
                self.status = "Dynamic simulation · OGC contacts".into();
                self.metrics = Some(metrics);
                Some(Ok(()))
            }
            Err(e) => {
                self.playing = false;
                self.simulation_error = Some(e.clone());
                self.status = e.clone();
                Some(Err(e))
            }
        }
    }

    #[inline]
    pub fn piece_summaries(&self) -> Vec<PieceSummary> {
        piece_summaries(&self.project, &self.scene)
    }

    #[inline]
    pub fn seam_summaries(&self) -> Vec<SeamSummary> {
        seam_summaries(&self.project)
    }

    #[inline]
    pub fn material_summaries(&self) -> Vec<MaterialSummary> {
        material_summaries(&self.project)
    }

    #[inline]
    pub fn trim_summaries(&self) -> Vec<TrimSummary> {
        trim_summaries(&self.project)
    }

    #[inline]
    pub fn pin_summaries(&self) -> Vec<PinSummary> {
        pin_summaries(&self.project)
    }

    #[inline]
    pub fn measurement_summaries(&self) -> Vec<MeasurementSummary> {
        measurement_summaries(&self.project)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn make_test_core() -> (tempfile::TempDir, AtelierCore) {
        let dir = tempfile::tempdir().unwrap();
        let avatar = dir.path().join("avatar.obj");
        std::fs::write(&avatar, "v 0 0 -10\nv 1 0 -10\nv 0 1 -10\nf 1 2 3\n").unwrap();
        let project = FittingProject::basic(GarmentKind::Tshirt, 0, avatar);
        let core = AtelierCore::from_project(project).expect("core compile");
        (dir, core)
    }

    #[test]
    fn test_repaint_signal_notifies() {
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        let signal = RepaintSignal::from_fn(move || {
            fired_clone.store(true, Ordering::SeqCst);
        });
        assert!(!fired.load(Ordering::SeqCst));
        signal.request_repaint();
        assert!(fired.load(Ordering::SeqCst));

        let noop = RepaintSignal::noop();
        noop.request_repaint();
    }

    #[test]
    fn test_core_headless_inspection_and_summaries() {
        let (_dir, core) = make_test_core();
        assert!(!core.project().template.panels.is_empty());
        assert!(!core.compiled_garment().panels.is_empty());
        assert!(!core.state().x.is_empty());
        assert!(core.settings().dt > 0.0);
        assert_eq!(core.active_tool(), Tool::Edit);
        assert_eq!(core.selection().primary_panel, Some(core.project().template.panels[0].id.clone()));
        assert!(!core.selection().is_empty());
        assert!(!core.has_pending_trim_draft());
        assert!(!core.has_pending_measurement_draft());
        assert!(!core.playing());

        let pieces = core.piece_summaries();
        assert_eq!(pieces.len(), core.project().template.panels.len());
        for p in &pieces {
            assert!(!p.id.is_empty());
            assert!(p.vertex_count > 0);
        }

        let seams = core.seam_summaries();
        assert_eq!(seams.len(), core.project().template.seams.len());

        let materials = core.material_summaries();
        assert_eq!(materials.len(), core.project().template.fabrics.len());

        let trims = core.trim_summaries();
        assert_eq!(trims.len(), core.project().template.trims.len());

        let pins = core.pin_summaries();
        assert_eq!(pins.len(), core.project().template.pin_targets.len());

        let measurements = core.measurement_summaries();
        assert!(measurements.is_empty());
    }

    #[test]
    fn test_core_command_availability_and_execution() {
        let (_dir, mut core) = make_test_core();

        // Toggle simulation command
        assert_eq!(core.command_availability(CommandId::ToggleSimulation), Ok(()));

        // When selection is cleared, tool move is disabled
        core.selection_mut().clear();
        assert_eq!(core.selection().primary_panel, None);
        assert_eq!(
            core.command_availability(CommandId::ToolMove),
            Err(UnavailableReason::NoSelectedPanel)
        );

        // Select panel restores tool move
        let first_id = core.project().template.panels[0].id.clone();
        core.selection_mut().select_panel(&first_id);
        assert_eq!(core.command_availability(CommandId::ToolMove), Ok(()));

        // Execute tool switch
        assert!(core.execute_command(CommandId::ToolRectangle).is_ok());
        assert_eq!(core.active_tool(), Tool::Rectangle);

        // Pending trim draft guard prevents reset and reload
        core.set_pending_trim_draft(true);
        assert!(core.has_pending_trim_draft());
        let reset_err = core.execute_command(CommandId::ResetSimulation).unwrap_err();
        assert!(reset_err.contains("trim draft"));
        let load_err = core.execute_command(CommandId::LoadTshirt).unwrap_err();
        assert!(load_err.contains("trim draft"));
        core.set_pending_trim_draft(false);

        // Pending measurement draft guard prevents reset and reload
        core.set_pending_measurement_draft(true);
        assert!(core.has_pending_measurement_draft());
        let reset_meas_err = core.execute_command(CommandId::ResetSimulation).unwrap_err();
        assert!(reset_meas_err.contains("measurement draft"));
        let load_meas_err = core.execute_command(CommandId::LoadTshirt).unwrap_err();
        assert!(load_meas_err.contains("measurement draft"));
        core.set_pending_measurement_draft(false);

        // Clearing draft guard allows reset
        assert!(core.execute_command(CommandId::ResetSimulation).is_ok());

        // Pausing simulation sends a paused Work packet to the worker
        assert!(core.set_playing(true).is_ok());
        assert!(core.playing());
        assert!(core.set_playing(false).is_ok());
        assert!(!core.playing());
    }

    #[test]
    fn test_core_diagnostics_request() {
        let (_dir, core) = make_test_core();
        let req = core.request_final_diagnostics();
        assert_eq!(req.generation, core.generation);
        assert!(req.request_id > 0);
    }
}

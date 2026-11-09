//! Property Editor Part 2 in `cssimpler`:
//! - Fabric & Appearance (FC-0004)
//! - Construction Trims & Repeat Controls (FC-0007, FC-0008)
//! - Physics-Inspired Mesh Preparation (FC-0006)
//! - Simulation & Avatar Collision Diagnostics
//!
//! SI units throughout: metres, kilograms, seconds, Newtons.

use cssimpler::core::Node;
use cssimpler::ui;
use marvelous_cloth::{
    avatar::{SurfaceMesh, diagnostics::Report as AvatarReport},
    fabrics::PRESETS,
    garment::{
        GarmentTemplate,
        appearance::FabricAppearance,
        template::{FabricProperties, Panel},
        trims::TrimKind,
    },
    vbd::{FinalDiagnostics, FinalDiagnosticsBackend, VbdSettings},
};

// ============================================================================
// 1. Fabric & Appearance (FC-0004)
// ============================================================================

/// State for the `Fabric & Appearance` accordion.
#[derive(Clone, Debug, PartialEq)]
pub struct FabricAppearanceState {
    pub panel_id: Option<String>,
    pub fabric_id: Option<String>,
    pub fabric_name: String,
    pub preset_index: Option<usize>,
    /// SI density in kg/m² (displayed with g/m² equivalent).
    pub density_kg_m2: f64,
    /// Warp membrane stiffness in N/m.
    pub warp_stiffness_n_m: f64,
    /// Weft membrane stiffness in N/m.
    pub weft_stiffness_n_m: f64,
    /// In-plane shear stiffness in N/m.
    pub shear_stiffness_n_m: f64,
    /// Bending stiffness warp in N·m.
    pub bend_warp_n_m: f64,
    /// Bending stiffness weft in N·m.
    pub bend_weft_n_m: f64,
    /// Fabric thickness in mm.
    pub thickness_mm: f64,
    /// Authored base tint RGB bytes (sRGB 0..=255).
    pub color_srgb: [u8; 3],
    /// Dimensionless perceptual roughness in [0.0, 1.0].
    pub roughness: f64,
    /// Portable albedo texture path/status.
    pub albedo_texture_path: Option<String>,
    pub texture_status: String,
    /// Physical repeat scale in millimetres (width_mm, height_mm).
    pub texture_repeat_mm: [f64; 2],
    /// UV offset in millimetres.
    pub texture_uv_offset_mm: [f64; 2],
    /// Rotation in degrees around material-space origin.
    pub texture_rotation_deg: f64,
    pub custom_appearance_enabled: bool,
    pub error_message: Option<String>,
}

impl FabricAppearanceState {
    pub const TEXTURE_DISCLAIMER: &'static str = "Appearance textures are visual only and do not alter physical mass, density, or stiffness.";

    pub fn from_fabric_and_appearance(
        panel_id: Option<String>,
        fabric_id: Option<String>,
        fabric_name: String,
        properties: &FabricProperties,
        appearance: Option<&FabricAppearance>,
    ) -> Self {
        let preset_index = PRESETS.iter().position(|p| p.properties() == *properties);

        let (custom, color, roughness, albedo_path, status, repeat_mm, rot_deg) =
            if let Some(app) = appearance {
                let (path, stat, rep, rot) = if let Some(tex) = &app.albedo {
                    (
                        Some(format!("Embedded PNG ({}×{})", tex.width, tex.height)),
                        format!("Loaded ({}×{} sRGB)", tex.width, tex.height),
                        [tex.tile_m[0] * 1000.0, tex.tile_m[1] * 1000.0],
                        tex.rotation_degrees,
                    )
                } else {
                    (None, "No texture loaded".to_string(), [100.0, 100.0], 0.0)
                };
                (true, app.color_srgb, app.roughness, path, stat, rep, rot)
            } else {
                (
                    false,
                    [204, 204, 204],
                    0.8,
                    None,
                    "No texture loaded".to_string(),
                    [100.0, 100.0],
                    0.0,
                )
            };

        Self {
            panel_id,
            fabric_id,
            fabric_name,
            preset_index,
            density_kg_m2: properties.density,
            warp_stiffness_n_m: properties.warp,
            weft_stiffness_n_m: properties.weft,
            shear_stiffness_n_m: properties.shear,
            bend_warp_n_m: properties.bend_warp,
            bend_weft_n_m: properties.bend_weft,
            thickness_mm: properties.thickness * 1000.0,
            color_srgb: color,
            roughness,
            albedo_texture_path: albedo_path,
            texture_status: status,
            texture_repeat_mm: repeat_mm,
            texture_uv_offset_mm: [0.0, 0.0],
            texture_rotation_deg: rot_deg,
            custom_appearance_enabled: custom,
            error_message: None,
        }
    }
}

// ============================================================================
// 2. Construction Trims & Repeat Controls (FC-0007, FC-0008)
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct TrimItemView {
    pub id: String,
    pub panel_id: String,
    pub kind_label: &'static str,
    pub dimensions_label: String,
    pub offset_mm: [f64; 2],
    pub color_srgb: [u8; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrimRepeatState {
    pub enabled: bool,
    /// Bounded repeat count in [1..=128].
    pub count: usize,
    /// Repeat spacing in millimetres [dx_mm, dy_mm].
    pub spacing_mm: [f64; 2],
}

impl Default for TrimRepeatState {
    fn default() -> Self {
        Self {
            enabled: false,
            count: 3,
            spacing_mm: [20.0, 0.0],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingTrimDraft {
    pub trim_id: String,
    pub panel_id: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrimsState {
    pub panel_id: Option<String>,
    pub trims: Vec<TrimItemView>,
    pub selected_trim_id: Option<String>,
    pub selected_kind_index: usize,
    pub repeat: TrimRepeatState,
    pub board_drag_active: bool,
    pub board_drag_status: String,
    pub pending_draft: Option<PendingTrimDraft>,
    pub warning: Option<String>,
}

impl TrimsState {
    pub const PENDING_DRAFT_WARNING: &'static str =
        "Apply or cancel the trim draft before opening Commands or resetting the simulation";

    pub fn from_template_and_panel(
        template: &GarmentTemplate,
        panel: Option<&Panel>,
        pending_draft: Option<PendingTrimDraft>,
        board_drag_active: bool,
    ) -> Self {
        let panel_id = panel.map(|p| p.id.clone());
        let trims = if let Some(p) = panel {
            template
                .trims
                .iter()
                .filter(|t| t.panel_id == p.id)
                .map(|t| {
                    let (kind_label, dim) = match &t.kind {
                        TrimKind::Button { diameter_m, .. } => {
                            ("Button", format!("⌀ {:.1} mm", diameter_m * 1000.0))
                        }
                        TrimKind::Buttonhole { width_m, .. } => {
                            ("Buttonhole", format!("width {:.1} mm", width_m * 1000.0))
                        }
                        TrimKind::Zipper { width_m, .. } => {
                            ("Zipper", format!("width {:.1} mm", width_m * 1000.0))
                        }
                        TrimKind::Topstitch {
                            rows, spacing_m, ..
                        } => (
                            "Topstitch",
                            format!("{} rows · {:.1} mm", rows, spacing_m * 1000.0),
                        ),
                    };
                    TrimItemView {
                        id: t.id.clone(),
                        panel_id: t.panel_id.clone(),
                        kind_label,
                        dimensions_label: dim,
                        offset_mm: [0.0, 0.0],
                        color_srgb: [50, 50, 50],
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let drag_status = if board_drag_active {
            "Active: dragging on 2D pattern board".to_string()
        } else {
            "Inactive".to_string()
        };

        Self {
            panel_id,
            trims,
            selected_trim_id: None,
            selected_kind_index: 0,
            repeat: TrimRepeatState::default(),
            board_drag_active,
            board_drag_status: drag_status,
            pending_draft,
            warning: None,
        }
    }
}

// ============================================================================
// 3. Physics-Inspired Mesh Preparation (FC-0006)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshScope {
    AllPolygonPieces,
    SelectedPolygonPiece,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshPreparationState {
    pub uniform_mesh_mm: f64,
    pub scope: MeshScope,
    pub selected_panel_id: Option<String>,
    /// Young's modulus estimates in Pa.
    pub warp_young_pa: f64,
    pub weft_young_pa: f64,
    pub boundary_ratio: f64,
    pub transition_distance_mm: f64,
    pub estimated_base_spacing_mm: Option<[f64; 2]>,
    pub simulation_worker_busy: bool,
    pub disabled_explanation: Option<String>,
    pub error: Option<String>,
}

impl MeshPreparationState {
    pub const WORKER_BUSY_EXPLANATION: &'static str =
        "Pause simulation and wait for the current step before remeshing or changing resolution.";

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uniform_mesh_mm: f64,
        scope: MeshScope,
        selected_panel_id: Option<String>,
        warp_young_pa: f64,
        weft_young_pa: f64,
        boundary_ratio: f64,
        transition_distance_mm: f64,
        simulation_worker_busy: bool,
    ) -> Self {
        let disabled_explanation = if simulation_worker_busy {
            Some(Self::WORKER_BUSY_EXPLANATION.to_string())
        } else {
            None
        };

        Self {
            uniform_mesh_mm,
            scope,
            selected_panel_id,
            warp_young_pa,
            weft_young_pa,
            boundary_ratio,
            transition_distance_mm,
            estimated_base_spacing_mm: Some([12.5, 12.5]),
            simulation_worker_busy,
            disabled_explanation,
            error: None,
        }
    }
}

// ============================================================================
// 4. Simulation, Final-State Audit & Avatar Collision Diagnostics
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct VbdSettingsView {
    pub dt_ms: f64,
    pub iterations: usize,
    pub damping: f64,
}

impl From<&VbdSettings> for VbdSettingsView {
    fn from(s: &VbdSettings) -> Self {
        Self {
            dt_ms: s.dt * 1000.0,
            iterations: s.iterations,
            damping: s.damping,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinalDiagnosticsView {
    pub state_time_s: f64,
    pub step_dt_ms: f64,
    pub sample_total_ms: f64,
    pub detection_ms: f64,
    pub residual_ms: f64,
    pub active_contacts: usize,
    pub retained_contact_terms: usize,
    pub minimum_separation_mm: Option<f64>,
    pub free_vertices: usize,
    pub residual_rms_n: Option<f64>,
    pub backend_badge: &'static str,
    pub freshness_badge: &'static str,
    pub semantics_note: &'static str,
}

impl FinalDiagnosticsView {
    pub fn from_diagnostics(sample: &FinalDiagnostics, current_time_s: f64, playing: bool) -> Self {
        let age_s = current_time_s - sample.state_time_s;
        let freshness = if age_s.abs() <= 1e-6 {
            if playing {
                "fresh at the sampled step"
            } else {
                "fresh (paused at the sampled state)"
            }
        } else if age_s < 0.0 {
            "stale (simulation state reset or reloaded)"
        } else {
            "stale; pause and sample again to inspect the current state"
        };

        let backend = match sample.backend {
            FinalDiagnosticsBackend::Cpu => "CPU step / CPU audit",
            FinalDiagnosticsBackend::CpuAuditOfGpuState => {
                "GPU-accepted state / CPU audit (GPU timings unavailable)"
            }
        };

        Self {
            state_time_s: sample.state_time_s,
            step_dt_ms: sample.step_dt_s * 1000.0,
            sample_total_ms: sample.sample_ms,
            detection_ms: sample.detection_ms,
            residual_ms: sample.residual_ms,
            active_contacts: sample.contacts,
            retained_contact_terms: sample.contact_terms,
            minimum_separation_mm: sample.minimum_distance_m.map(|d| d * 1000.0),
            free_vertices: sample.free_vertices,
            residual_rms_n: sample.residual_rms_n,
            backend_badge: backend,
            freshness_badge: freshness,
            semantics_note: "Accepted post-step state with fresh final OGC contacts. Unconstrained free-DOF backward-Euler gradient, not a KKT/constrained-equilibrium or per-color mixed-gradient certificate.",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AvatarDiagnosticsView {
    pub vertex_count: usize,
    pub used_vertex_count: usize,
    pub triangle_count: usize,
    pub dimensions_m: [f64; 3],
    pub min_bounds_m: [f64; 3],
    pub max_bounds_m: [f64; 3],
    pub open_edges: usize,
    pub nonmanifold_edges: usize,
    pub inconsistent_edges: usize,
    pub duplicate_triangles: usize,
    pub fingerprint: String,
    pub initialization_gate_result: Option<Result<String, String>>,
    pub vbd_ogc_radius_mm: f64,
    pub penalty_clearance_mm: f64,
    pub penalty_stiffness_n_m: f64,
    pub disclaimer: &'static str,
}

impl AvatarDiagnosticsView {
    pub const DISCLAIMER: &'static str = "Indexed topology only; no welding or avatar self-intersection test. Start checks use production sewing exclusions and do not certify global inside/outside containment. No repairs or solver changes.";

    pub fn from_report(
        report: &AvatarReport,
        settings: &VbdSettings,
        penalty_clearance_m: f64,
        penalty_stiffness: f64,
        initialization_gate: Option<Result<String, String>>,
    ) -> Self {
        let size = [
            report.max.x - report.min.x,
            report.max.y - report.min.y,
            report.max.z - report.min.z,
        ];

        Self {
            vertex_count: report.vertices,
            used_vertex_count: report.used_vertices,
            triangle_count: report.triangles,
            dimensions_m: size,
            min_bounds_m: [report.min.x, report.min.y, report.min.z],
            max_bounds_m: [report.max.x, report.max.y, report.max.z],
            open_edges: report.boundary_edges,
            nonmanifold_edges: report.nonmanifold_edges,
            inconsistent_edges: report.inconsistent_two_face_edges,
            duplicate_triangles: report.duplicate_triangles,
            fingerprint: report.fingerprint.clone(),
            initialization_gate_result: initialization_gate,
            vbd_ogc_radius_mm: settings.contact.radius * 1000.0,
            penalty_clearance_mm: penalty_clearance_m * 1000.0,
            penalty_stiffness_n_m: penalty_stiffness,
            disclaimer: Self::DISCLAIMER,
        }
    }

    pub fn from_surface_and_settings(
        surface: &SurfaceMesh,
        settings: &VbdSettings,
        penalty_clearance_m: f64,
        penalty_stiffness: f64,
        initialization_gate: Option<Result<String, String>>,
    ) -> Result<Self, String> {
        let report = marvelous_cloth::avatar::diagnostics::inspect(surface)?;
        Ok(Self::from_report(
            &report,
            settings,
            penalty_clearance_m,
            penalty_stiffness,
            initialization_gate,
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimulationDiagnosticsState {
    pub vbd_settings: VbdSettingsView,
    pub final_diagnostics: Option<FinalDiagnosticsView>,
    pub final_sample_pending: bool,
    pub avatar_diagnostics: Option<AvatarDiagnosticsView>,
    pub inspect_pending: bool,
    pub check_initialization_pending: bool,
    pub simulation_playing_or_worker_pending: bool,
    pub error: Option<String>,
}

// ============================================================================
// Overall Container & Accordion Open State
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct AccordionOpenState {
    pub fabric_appearance: bool,
    pub trims: bool,
    pub mesh_preparation: bool,
    pub simulation_avatar_diagnostics: bool,
}

impl Default for AccordionOpenState {
    fn default() -> Self {
        Self {
            fabric_appearance: true,
            trims: true,
            mesh_preparation: true,
            simulation_avatar_diagnostics: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertiesPart2State {
    pub fabric: FabricAppearanceState,
    pub trims: TrimsState,
    pub mesh_prep: MeshPreparationState,
    pub sim_diag: SimulationDiagnosticsState,
    pub accordions: AccordionOpenState,
}

// ============================================================================
// UI Rendering Functions with cssimpler::ui!
// ============================================================================

/// Render the complete Property Editor Part 2 panel containing all four accordions.
pub fn render_properties_part2(state: &PropertiesPart2State) -> Node {
    let fabric_node =
        render_fabric_appearance_accordion(&state.fabric, state.accordions.fabric_appearance);
    let trims_node = render_trims_accordion(&state.trims, state.accordions.trims);
    let mesh_node =
        render_mesh_preparation_accordion(&state.mesh_prep, state.accordions.mesh_preparation);
    let sim_node = render_simulation_diagnostics_accordion(
        &state.sim_diag,
        state.accordions.simulation_avatar_diagnostics,
    );

    ui! {
        <aside id="property-editor-part2" class="property-editor-panel">
            <header class="panel-header">
                <h2 class="panel-title">Properties</h2>
            </header>
            <div class="accordions-container">
                {fabric_node}
                {trims_node}
                {mesh_node}
                {sim_node}
            </div>
        </aside>
    }
}

/// Render Accordion 1: Fabric & Appearance.
pub fn render_fabric_appearance_accordion(state: &FabricAppearanceState, is_open: bool) -> Node {
    let open_class = if is_open {
        "accordion-open"
    } else {
        "accordion-closed"
    };
    let density_text = format!(
        "{:.3} kg/m² ({:.0} g/m²)",
        state.density_kg_m2,
        state.density_kg_m2 * 1000.0
    );
    let warp_text = format!("{:.0} N/m", state.warp_stiffness_n_m);
    let weft_text = format!("{:.0} N/m", state.weft_stiffness_n_m);
    let shear_text = format!("{:.0} N/m", state.shear_stiffness_n_m);
    let bend_warp_text = format!("{:.2e} N·m", state.bend_warp_n_m);
    let bend_weft_text = format!("{:.2e} N·m", state.bend_weft_n_m);
    let thickness_text = format!("{:.2} mm", state.thickness_mm);

    let [r, g, b] = state.color_srgb;
    let rgb_css = format!("rgb({}, {}, {})", r, g, b);
    let r_str = r.to_string();
    let g_str = g.to_string();
    let b_str = b.to_string();
    let roughness_str = format!("{:.2}", state.roughness);

    let repeat_w_str = format!("{:.1}", state.texture_repeat_mm[0]);
    let repeat_h_str = format!("{:.1}", state.texture_repeat_mm[1]);
    let rot_str = format!("{:.1}", state.texture_rotation_deg);

    let preset_name = state
        .preset_index
        .and_then(|idx| PRESETS.get(idx))
        .map(|p| p.name)
        .unwrap_or("Custom / Choose preset");

    ui! {
        <section id="accordion-fabric-appearance" class={format!("accordion-section {}", open_class)}>
            <div class="accordion-header" id="header-fabric-appearance">
                <span class="accordion-toggle-icon">{if is_open { "▼" } else { "▶" }}</span>
                <span class="accordion-title">Fabric & Appearance</span>
            </div>
            <div class="accordion-body">
                <div class="form-row preset-selector">
                    <label class="form-label">Fabric Preset</label>
                    <span class="preset-name" id="selected-fabric-preset">{preset_name}</span>
                </div>
                <div class="section-subheading">Mechanical Properties (SI)</div>
                <div class="property-grid">
                    <div class="grid-item">
                        <span class="item-label">Area Density:</span>
                        <span class="item-value" id="val-fabric-density">{density_text}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Warp Stiffness:</span>
                        <span class="item-value" id="val-fabric-warp">{warp_text}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Weft Stiffness:</span>
                        <span class="item-value" id="val-fabric-weft">{weft_text}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Shear Stiffness:</span>
                        <span class="item-value" id="val-fabric-shear">{shear_text}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Bending Warp:</span>
                        <span class="item-value" id="val-fabric-bend-warp">{bend_warp_text}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Bending Weft:</span>
                        <span class="item-value" id="val-fabric-bend-weft">{bend_weft_text}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Thickness:</span>
                        <span class="item-value" id="val-fabric-thickness">{thickness_text}</span>
                    </div>
                </div>

                <div class="section-subheading">Optics & Albedo Appearance</div>
                <div class="form-row color-tint-row">
                    <label class="form-label">Base Tint (RGB)</label>
                    <div class="color-swatch-box" style={format!("background-color: {};", rgb_css)}></div>
                    <div class="rgb-inputs">
                        <input type="text" class="rgb-input" id="input-tint-r" value={r_str} />
                        <input type="text" class="rgb-input" id="input-tint-g" value={g_str} />
                        <input type="text" class="rgb-input" id="input-tint-b" value={b_str} />
                    </div>
                </div>
                <div class="form-row roughness-row">
                    <label class="form-label">Roughness</label>
                    <input type="text" class="number-input" id="input-roughness" value={roughness_str} />
                </div>
                <div class="form-row texture-status-row">
                    <label class="form-label">Albedo Texture</label>
                    <span class="texture-status-badge" id="texture-status">{state.texture_status.clone()}</span>
                </div>
                <div class="form-row texture-repeat-row">
                    <label class="form-label">Repeat Scale (mm)</label>
                    <div class="repeat-inputs">
                        <input type="text" class="number-input" id="input-repeat-w" value={repeat_w_str} />
                        <span class="unit-separator">{ "×" }</span>
                        <input type="text" class="number-input" id="input-repeat-h" value={repeat_h_str} />
                        <span class="unit-suffix">mm</span>
                    </div>
                </div>
                <div class="form-row texture-rotation-row">
                    <label class="form-label">Rotation</label>
                    <div class="rotation-inputs">
                        <input type="text" class="number-input" id="input-rotation" value={rot_str} />
                        <span class="unit-suffix">{ "°" }</span>
                    </div>
                </div>
                <div class="button-row texture-actions">
                    <button class="btn btn-secondary" id="btn-load-texture">Load Texture</button>
                    <button class="btn btn-danger" id="btn-clear-texture">Clear Texture</button>
                </div>
                <p class="disclaimer-note" id="fabric-appearance-disclaimer">
                    {FabricAppearanceState::TEXTURE_DISCLAIMER}
                </p>
            </div>
        </section>
    }
}

/// Render Accordion 2: Construction Trims & Repeat Controls.
pub fn render_trims_accordion(state: &TrimsState, is_open: bool) -> Node {
    let open_class = if is_open {
        "accordion-open"
    } else {
        "accordion-closed"
    };

    // Pending draft warning banner per FC-0008:
    let banner_node = if let Some(draft) = &state.pending_draft {
        ui! {
            <div class="warning-banner trim-draft-guard-banner" id="trim-draft-guard-banner">
                <span class="warning-icon">{ "⚠" }</span>
                <p class="warning-message" id="trim-draft-warning-text">
                    {format!("{}: draft for trim '{}' on piece '{}'", TrimsState::PENDING_DRAFT_WARNING, draft.trim_id, draft.panel_id)}
                </p>
                <div class="banner-actions">
                    <button class="btn btn-primary btn-sm" id="btn-apply-trim-draft">Apply Draft</button>
                    <button class="btn btn-secondary btn-sm" id="btn-discard-trim-draft">Discard Draft</button>
                </div>
            </div>
        }
    } else {
        ui! { <div class="no-banner"></div> }
    };

    let repeat_count_str = state.repeat.count.to_string();
    let repeat_dx_str = format!("{:.1}", state.repeat.spacing_mm[0]);
    let repeat_dy_str = format!("{:.1}", state.repeat.spacing_mm[1]);

    let trim_items_nodes: Vec<Node> = state
        .trims
        .iter()
        .map(|t| {
            let trim_id_clone = t.id.clone();
            ui! {
                <li class="trim-list-item" id={format!("trim-item-{}", trim_id_clone)}>
                    <span class="trim-kind">{t.kind_label}</span>
                    <span class="trim-id">{format!("#{}", t.id)}</span>
                    <span class="trim-dim">{t.dimensions_label.clone()}</span>
                    <div class="trim-item-actions">
                        <button class="btn btn-xs" id={format!("btn-duplicate-trim-{}", t.id)}>Duplicate</button>
                        <button class="btn btn-xs btn-danger" id={format!("btn-delete-trim-{}", t.id)}>Delete</button>
                    </div>
                </li>
            }
        })
        .collect();

    ui! {
        <section id="accordion-trims" class={format!("accordion-section {}", open_class)}>
            <div class="accordion-header" id="header-trims">
                <span class="accordion-toggle-icon">{if is_open { "▼" } else { "▶" }}</span>
                <span class="accordion-title">Construction Trims & Repeat</span>
            </div>
            <div class="accordion-body">
                {banner_node}

                <div class="board-drag-indicator" id="board-drag-status-indicator">
                    <span class="status-label">2D Board Drag:</span>
                    <span class="status-value" id="val-board-drag-status">{state.board_drag_status.clone()}</span>
                </div>

                <div class="section-subheading">Attached Trims</div>
                <ul class="trim-items-list" id="attached-trims-list">
                    {trim_items_nodes}
                </ul>

                <div class="section-subheading">Repeat Controls (TrimRepeat)</div>
                <div class="form-row repeat-count-row">
                    <label class="form-label">Copies (1..=128)</label>
                    <input type="text" class="number-input" id="input-repeat-count" value={repeat_count_str} />
                </div>
                <div class="form-row repeat-spacing-row">
                    <label class="form-label">Spacing X / Y (mm)</label>
                    <div class="spacing-inputs">
                        <input type="text" class="number-input" id="input-repeat-dx" value={repeat_dx_str} />
                        <span class="unit-separator">,</span>
                        <input type="text" class="number-input" id="input-repeat-dy" value={repeat_dy_str} />
                        <span class="unit-suffix">mm</span>
                    </div>
                </div>
                <div class="button-row repeat-actions">
                    <button class="btn btn-primary btn-sm" id="btn-apply-repeat">Apply Repeat</button>
                    <button class="btn btn-secondary btn-sm" id="btn-cancel-repeat">Cancel Repeat</button>
                </div>
            </div>
        </section>
    }
}

/// Render Accordion 3: Physics-Inspired Mesh Preparation.
pub fn render_mesh_preparation_accordion(state: &MeshPreparationState, is_open: bool) -> Node {
    let open_class = if is_open {
        "accordion-open"
    } else {
        "accordion-closed"
    };
    let uniform_res_str = format!("{:.1}", state.uniform_mesh_mm);
    let warp_pa_str = format!("{:.0}", state.warp_young_pa);
    let weft_pa_str = format!("{:.0}", state.weft_young_pa);
    let ratio_str = format!("{:.2}", state.boundary_ratio);
    let trans_str = format!("{:.1}", state.transition_distance_mm);

    let scope_text = match state.scope {
        MeshScope::AllPolygonPieces => "All polygon pieces",
        MeshScope::SelectedPolygonPiece => "Selected polygon piece",
    };

    let base_spacing_text = if let Some([bw, bh]) = state.estimated_base_spacing_mm {
        format!("Estimated base spacing: {:.2} × {:.2} mm", bw, bh)
    } else {
        "Base spacing estimate unavailable".to_string()
    };

    let busy = state.simulation_worker_busy;
    let button_disabled_class = if busy { "btn-disabled" } else { "" };
    let disabled_attr = if busy { "disabled" } else { "enabled" };

    let explanation_node = if let Some(exp) = &state.disabled_explanation {
        ui! {
            <p class="guard-explanation-message" id="mesh-prep-disabled-explanation">
                {exp.clone()}
            </p>
        }
    } else {
        ui! { <div class="no-explanation"></div> }
    };

    ui! {
        <section id="accordion-mesh-preparation" class={format!("accordion-section {}", open_class)}>
            <div class="accordion-header" id="header-mesh-preparation">
                <span class="accordion-toggle-icon">{if is_open { "▼" } else { "▶" }}</span>
                <span class="accordion-title">Physics-Inspired Mesh Preparation</span>
            </div>
            <div class="accordion-body">
                <div class="form-row uniform-res-row">
                    <label class="form-label">Uniform Resolution (mm)</label>
                    <input type="text" class="number-input" id="input-uniform-resolution" value={uniform_res_str} />
                    <span class="unit-suffix">mm</span>
                </div>
                <div class="form-row scope-row">
                    <label class="form-label">Scope</label>
                    <span class="scope-value" id="mesh-prep-scope">{scope_text}</span>
                </div>
                <div class="section-subheading">{ "Young's Modulus Sizing Estimates (Pa)" }</div>
                <div class="form-row young-warp-row">
                    <label class="form-label">Warp E (Pa)</label>
                    <input type="text" class="number-input" id="input-young-warp" value={warp_pa_str} />
                </div>
                <div class="form-row young-weft-row">
                    <label class="form-label">Weft E (Pa)</label>
                    <input type="text" class="number-input" id="input-young-weft" value={weft_pa_str} />
                </div>
                <div class="form-row boundary-ratio-row">
                    <label class="form-label">Boundary / Base Spacing</label>
                    <input type="text" class="number-input" id="input-boundary-ratio" value={ratio_str} />
                </div>
                <div class="form-row transition-dist-row">
                    <label class="form-label">Transition Distance (mm)</label>
                    <input type="text" class="number-input" id="input-transition-distance" value={trans_str} />
                    <span class="unit-suffix">mm</span>
                </div>
                <div class="spacing-estimate-box" id="mesh-prep-base-spacing">
                    <span class="estimate-text">{base_spacing_text}</span>
                </div>

                {explanation_node}

                <div class="button-row mesh-prep-actions">
                    <button class={format!("btn btn-primary {}", button_disabled_class)} id="btn-remesh-and-reset" disabled={disabled_attr}>
                        Remesh and reset
                    </button>
                    <button class={format!("btn btn-secondary {}", button_disabled_class)} id="btn-uniform-mesh-and-reset" disabled={disabled_attr}>
                        Use uniform grid and reset
                    </button>
                </div>
            </div>
        </section>
    }
}

/// Render Accordion 4: Simulation, Final-State Audit & Avatar Collision Diagnostics.
pub fn render_simulation_diagnostics_accordion(
    state: &SimulationDiagnosticsState,
    is_open: bool,
) -> Node {
    let open_class = if is_open {
        "accordion-open"
    } else {
        "accordion-closed"
    };
    let dt_str = format!("{:.3} ms", state.vbd_settings.dt_ms);
    let iter_str = state.vbd_settings.iterations.to_string();
    let damp_str = format!("{:.2}", state.vbd_settings.damping);

    let final_diag_node = if let Some(fd) = &state.final_diagnostics {
        let timing_text = format!(
            "Accepted state t = {:.3} s · dt = {:.3} ms · sample {:.2} ms (detect {:.2} ms + residual {:.2} ms)",
            fd.state_time_s, fd.step_dt_ms, fd.sample_total_ms, fd.detection_ms, fd.residual_ms
        );
        let contacts_text = match fd.minimum_separation_mm {
            Some(sep) => format!(
                "Contacts: {} active / {} retained · minimum separation {:.3} mm",
                fd.active_contacts, fd.retained_contact_terms, sep
            ),
            None => format!(
                "Contacts: {} active / {} retained · no contact term",
                fd.active_contacts, fd.retained_contact_terms
            ),
        };
        let residual_text = match fd.residual_rms_n {
            Some(rms) => format!(
                "Free-DOF RMS residual: {:.4e} N over {} free vertices",
                rms, fd.free_vertices
            ),
            None => "Free-DOF RMS residual unavailable: no free vertices".to_string(),
        };

        ui! {
            <div class="final-diagnostics-report" id="final-diagnostics-report">
                <div class="diag-line timing" id="diag-timing">{timing_text}</div>
                <div class="diag-line contacts" id="diag-contacts">{contacts_text}</div>
                <div class="diag-line residual" id="diag-residual">{residual_text}</div>
                <div class="badge-row">
                    <span class="badge backend-badge" id="badge-backend">{fd.backend_badge}</span>
                    <span class="badge freshness-badge" id="badge-freshness">{fd.freshness_badge}</span>
                </div>
                <p class="semantics-note" id="diag-semantics">{fd.semantics_note}</p>
            </div>
        }
    } else {
        ui! {
            <div class="final-diagnostics-empty" id="final-diagnostics-empty">
                <p class="empty-note">{ "No final-state sample yet. Click 'Sample final state' to audit." }</p>
            </div>
        }
    };

    let avatar_node = if let Some(ad) = &state.avatar_diagnostics {
        let counts_text = format!(
            "{} vertices ({} referenced) · {} triangles",
            ad.vertex_count, ad.used_vertex_count, ad.triangle_count
        );
        let dims_text = format!(
            "Size: {:.3} × {:.3} × {:.3} m",
            ad.dimensions_m[0], ad.dimensions_m[1], ad.dimensions_m[2]
        );
        let topology_text = format!(
            "Open edges: {} · Nonmanifold: {} · Inconsistent: {} · Duplicate faces: {}",
            ad.open_edges, ad.nonmanifold_edges, ad.inconsistent_edges, ad.duplicate_triangles
        );
        let radii_text = format!(
            "VBD / OGC contact radius: {:.3} mm · Penalty collider: clearance {:.3} mm, stiffness {:.2e} N/m",
            ad.vbd_ogc_radius_mm, ad.penalty_clearance_mm, ad.penalty_stiffness_n_m
        );

        let gate_text = match &ad.initialization_gate_result {
            Some(Ok(msg)) => format!("Initialization gate: PASSED ({})", msg),
            Some(Err(err)) => format!("Initialization gate: REJECTED ({})", err),
            None => "Initialization gate: not checked".to_string(),
        };

        ui! {
            <div class="avatar-diagnostics-report" id="avatar-diagnostics-report">
                <div class="avatar-stat counts" id="avatar-counts">{counts_text}</div>
                <div class="avatar-stat dimensions" id="avatar-dimensions">{dims_text}</div>
                <div class="avatar-stat topology" id="avatar-topology">{topology_text}</div>
                <div class="avatar-stat radii" id="avatar-radii">{radii_text}</div>
                <div class="avatar-stat fingerprint" id="avatar-fingerprint">{ad.fingerprint.clone()}</div>
                <div class="avatar-stat gate-status" id="avatar-gate-status">{gate_text}</div>
                <p class="avatar-disclaimer" id="avatar-disclaimer-text">{ad.disclaimer}</p>
            </div>
        }
    } else {
        ui! {
            <div class="avatar-diagnostics-empty" id="avatar-diagnostics-empty">
                <p class="empty-note">No collision mesh inspection performed yet.</p>
            </div>
        }
    };

    ui! {
        <section id="accordion-simulation-diagnostics" class={format!("accordion-section {}", open_class)}>
            <div class="accordion-header" id="header-simulation-diagnostics">
                <span class="accordion-toggle-icon">{if is_open { "▼" } else { "▶" }}</span>
                <span class="accordion-title">Simulation & Avatar Diagnostics</span>
            </div>
            <div class="accordion-body">
                <div class="section-subheading">VBD Solver Settings</div>
                <div class="property-grid solver-grid">
                    <div class="grid-item">
                        <span class="item-label">dt:</span>
                        <span class="item-value" id="val-solver-dt">{dt_str}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Iterations:</span>
                        <span class="item-value" id="val-solver-iterations">{iter_str}</span>
                    </div>
                    <div class="grid-item">
                        <span class="item-label">Damping:</span>
                        <span class="item-value" id="val-solver-damping">{damp_str}</span>
                    </div>
                </div>

                <div class="section-subheading">Final-State Audit</div>
                <div class="button-row">
                    <button class="btn btn-secondary btn-sm" id="btn-sample-final-state">Sample final state</button>
                </div>
                {final_diag_node}

                <div class="section-subheading">Avatar Collision Diagnostics</div>
                <div class="button-row">
                    <button class="btn btn-secondary btn-sm" id="btn-inspect-collision-mesh">Inspect collision mesh</button>
                    <button class="btn btn-secondary btn-sm" id="btn-check-initialization">Check current initialization</button>
                </div>
                {avatar_node}
            </div>
        </section>
    }
}

// ============================================================================
// Unit Tests (Instruction 3)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cssimpler::core::ElementNode;
    use glam::DVec2;
    use marvelous_cloth::garment::appearance::AlbedoTexture;
    use marvelous_cloth::garment::trims::{Trim, TrimKind};

    fn find_element_by_id<'a>(node: &'a Node, target_id: &str) -> Option<&'a ElementNode> {
        match node {
            Node::Element(el) => {
                if el.id.as_deref() == Some(target_id) {
                    return Some(el);
                }
                for child in &el.children {
                    if let Some(found) = find_element_by_id(child, target_id) {
                        return Some(found);
                    }
                }
                None
            }
            Node::Text(_) => None,
        }
    }

    fn collect_all_text(node: &Node) -> String {
        let mut out = String::new();
        match node {
            Node::Element(el) => {
                for child in &el.children {
                    out.push_str(&collect_all_text(child));
                    out.push(' ');
                }
            }
            Node::Text(t) => {
                out.push_str(t);
            }
        }
        out
    }

    #[test]
    fn test_fabric_appearance_rendering_with_units_and_disclaimer() {
        let mut properties = PRESETS[0].properties();
        properties.density = 0.245; // kg/m² -> 245 g/m²
        properties.warp = 1400.0; // N/m
        properties.weft = 1100.0; // N/m
        properties.shear = 380.0; // N/m
        properties.bend_warp = 1.8e-5; // N·m
        properties.bend_weft = 1.2e-5; // N·m
        properties.thickness = 0.00045; // 0.45 mm

        let appearance = FabricAppearance {
            color_srgb: [215, 190, 150],
            roughness: 0.62,
            albedo: Some(AlbedoTexture {
                width: 256,
                height: 256,
                rgba_srgb: vec![255; 256 * 256 * 4].into(),
                tile_m: [0.125, 0.125], // 125.0 mm × 125.0 mm
                rotation_degrees: 35.5,
            }),
        };

        let state = FabricAppearanceState::from_fabric_and_appearance(
            Some("panel-front".into()),
            Some("fabric-linen".into()),
            "Linen Blend".into(),
            &properties,
            Some(&appearance),
        );

        let node = render_fabric_appearance_accordion(&state, true);
        let all_text = collect_all_text(&node);

        // Verify mechanical SI units
        assert!(all_text.contains("0.245 kg/m²"));
        assert!(all_text.contains("245 g/m²"));
        assert!(all_text.contains("1400 N/m"));
        assert!(all_text.contains("1100 N/m"));
        assert!(all_text.contains("380 N/m"));
        assert!(all_text.contains("1.80e-5 N·m"));
        assert!(all_text.contains("0.45 mm"));

        // Verify texture status and physical repeat units
        assert!(all_text.contains("Loaded (256×256 sRGB)"));
        let repeat_w_el = find_element_by_id(&node, "input-repeat-w").expect("input-repeat-w");
        assert_eq!(
            repeat_w_el.attributes.get("value").map(|s| s.as_str()),
            Some("125.0")
        );
        let rot_el = find_element_by_id(&node, "input-rotation").expect("input-rotation");
        assert_eq!(
            rot_el.attributes.get("value").map(|s| s.as_str()),
            Some("35.5")
        );

        // Verify clear & load buttons
        assert!(find_element_by_id(&node, "btn-load-texture").is_some());
        assert!(find_element_by_id(&node, "btn-clear-texture").is_some());

        // Verify explicit disclaimer: appearance textures do not alter physical mass or stiffness
        assert!(all_text.contains(FabricAppearanceState::TEXTURE_DISCLAIMER));
    }

    #[test]
    fn test_construction_trims_and_repeat_guards() {
        let mut template = GarmentTemplate::two_panel_demo();
        let panel_id = template.panels[0].id.clone();
        template.trims = vec![
            Trim {
                id: "btn-01".into(),
                panel_id: panel_id.clone(),
                kind: TrimKind::Button {
                    center_m: DVec2::new(0.05, 0.10),
                    diameter_m: 0.018, // 18.0 mm
                },
            },
            Trim {
                id: "zip-01".into(),
                panel_id: panel_id.clone(),
                kind: TrimKind::Zipper {
                    path_m: vec![DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.3)],
                    width_m: 0.006, // 6.0 mm
                },
            },
        ];

        let panel = template.panels[0].clone();

        // Case 1: Trim draft is pending -> Warning banner must be present
        let pending = PendingTrimDraft {
            trim_id: "btn-01".into(),
            panel_id: panel_id.clone(),
            description: "Translating button on board".into(),
        };

        let state_pending =
            TrimsState::from_template_and_panel(&template, Some(&panel), Some(pending), true);
        let node_pending = render_trims_accordion(&state_pending, true);
        let text_pending = collect_all_text(&node_pending);

        // Warning banner and FC-0008 explanation must be rendered
        assert!(find_element_by_id(&node_pending, "trim-draft-guard-banner").is_some());
        assert!(find_element_by_id(&node_pending, "btn-apply-trim-draft").is_some());
        assert!(find_element_by_id(&node_pending, "btn-discard-trim-draft").is_some());
        assert!(text_pending.contains(TrimsState::PENDING_DRAFT_WARNING));
        assert!(text_pending.contains("Active: dragging on 2D pattern board"));

        // Case 2: No pending draft -> Warning banner must NOT be present
        let state_clean = TrimsState::from_template_and_panel(&template, Some(&panel), None, false);
        let node_clean = render_trims_accordion(&state_clean, true);
        assert!(find_element_by_id(&node_clean, "trim-draft-guard-banner").is_none());

        // Verify repeat controls (TrimRepeat)
        let repeat_count_el =
            find_element_by_id(&node_clean, "input-repeat-count").expect("input-repeat-count");
        assert_eq!(
            repeat_count_el.attributes.get("value").map(|s| s.as_str()),
            Some("3")
        );
        assert!(find_element_by_id(&node_clean, "btn-apply-repeat").is_some());
    }

    #[test]
    fn test_mesh_preparation_worker_guards() {
        // Case 1: Worker is running/busy -> Action buttons MUST be disabled with explanation
        let state_busy = MeshPreparationState::new(
            15.0,
            MeshScope::AllPolygonPieces,
            None,
            120_000.0,
            110_000.0,
            0.5,
            40.0,
            true, // simulation_worker_busy = true
        );

        let node_busy = render_mesh_preparation_accordion(&state_busy, true);
        let text_busy = collect_all_text(&node_busy);

        let btn_remesh_busy =
            find_element_by_id(&node_busy, "btn-remesh-and-reset").expect("btn-remesh");
        assert_eq!(
            btn_remesh_busy
                .attributes
                .get("disabled")
                .map(|s| s.as_str()),
            Some("disabled")
        );
        assert!(text_busy.contains(MeshPreparationState::WORKER_BUSY_EXPLANATION));

        // Case 2: Worker is idle -> Action buttons MUST be enabled
        let state_idle = MeshPreparationState::new(
            15.0,
            MeshScope::SelectedPolygonPiece,
            Some("panel-sleeve".into()),
            120_000.0,
            110_000.0,
            0.5,
            40.0,
            false, // simulation_worker_busy = false
        );

        let node_idle = render_mesh_preparation_accordion(&state_idle, true);
        let btn_remesh_idle =
            find_element_by_id(&node_idle, "btn-remesh-and-reset").expect("btn-remesh");
        assert_eq!(
            btn_remesh_idle
                .attributes
                .get("disabled")
                .map(|s| s.as_str()),
            Some("enabled")
        );
        assert!(find_element_by_id(&node_idle, "mesh-prep-disabled-explanation").is_none());
    }

    #[test]
    fn test_avatar_and_final_diagnostics_units_and_disclaimers() {
        let vbd_settings = VbdSettings::default();

        let final_diag = FinalDiagnostics {
            state_time_s: 2.500,
            step_dt_s: 0.016667,
            sample_ms: 3.25,
            detection_ms: 1.10,
            residual_ms: 2.15,
            backend: FinalDiagnosticsBackend::Cpu,
            contacts: 12,
            contact_terms: 36,
            minimum_distance_m: Some(0.00042), // 0.420 mm
            free_vertices: 640,
            residual_rms_n: Some(1.2345e-4), // N
            residual_kind: "free_dof_backward_euler",
        };

        let fd_view = FinalDiagnosticsView::from_diagnostics(&final_diag, 2.500, false);

        let avatar_diag = AvatarDiagnosticsView {
            vertex_count: 13380,
            used_vertex_count: 13380,
            triangle_count: 26756,
            dimensions_m: [0.450, 1.720, 0.280],
            min_bounds_m: [-0.225, 0.0, -0.140],
            max_bounds_m: [0.225, 1.720, 0.140],
            open_edges: 0,
            nonmanifold_edges: 0,
            inconsistent_edges: 0,
            duplicate_triangles: 0,
            fingerprint: "avatar-v1-mannequin-test".into(),
            initialization_gate_result: Some(Ok("No penetrating features detected".into())),
            vbd_ogc_radius_mm: 2.0,
            penalty_clearance_mm: 5.0,
            penalty_stiffness_n_m: 100_000.0,
            disclaimer: AvatarDiagnosticsView::DISCLAIMER,
        };

        let sim_state = SimulationDiagnosticsState {
            vbd_settings: (&vbd_settings).into(),
            final_diagnostics: Some(fd_view),
            final_sample_pending: false,
            avatar_diagnostics: Some(avatar_diag),
            inspect_pending: false,
            check_initialization_pending: false,
            simulation_playing_or_worker_pending: false,
            error: None,
        };

        let node = render_simulation_diagnostics_accordion(&sim_state, true);
        let all_text = collect_all_text(&node);

        // Verify FinalDiagnostics values and physical units
        assert!(all_text.contains("Accepted state t = 2.500 s"));
        assert!(all_text.contains("minimum separation 0.420 mm"));
        assert!(all_text.contains("1.2345e-4 N over 640 free vertices"));
        assert!(all_text.contains("CPU step / CPU audit"));
        assert!(all_text.contains("fresh (paused at the sampled state)"));

        // Verify AvatarDiagnostics values and disclaimer
        assert!(all_text.contains("13380 vertices (13380 referenced) · 26756 triangles"));
        assert!(all_text.contains("Size: 0.450 × 1.720 × 0.280 m"));
        assert!(all_text.contains("VBD / OGC contact radius: 2.000 mm"));
        assert!(all_text.contains("clearance 5.000 mm, stiffness 1.00e5 N/m"));
        assert!(all_text.contains("avatar-v1-mannequin-test"));
        assert!(all_text.contains("Initialization gate: PASSED"));
        assert!(all_text.contains(AvatarDiagnosticsView::DISCLAIMER));
    }
}

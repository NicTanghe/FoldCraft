//! CSS-based Property Editor Part 1: Information, Selected Line, Arrange, Measurements, and Pins.
//!
//! Connects numeric point constructions, notches/allowances, persistent ruler measurements (FC-0005),
//! and contour/interior pins (FC-0007) to cssimpler property accordions.

use crate::{
    authoring::{self, ConstructionRef, PointConstruction},
    garment::{
        markings::{NotchKind, polyline_length},
        polygon::{MeasurementAnchor, PatternMeasurement, PolygonPattern},
        seam::BoundaryRef,
        shapes::MirrorAxis,
        template::{Corner, GarmentTemplate, Panel, Placement},
    },
};
use glam::{DVec2, DVec3};

#[cfg(test)]
use cssimpler::core::ElementNode;
use cssimpler::core::Node;
use cssimpler::ui;

/// Mode for numeric point construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConstructionMode {
    #[default]
    Project,
    Distance,
    Perpendicular,
    Intersection,
}

impl ConstructionMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Distance => "Distance",
            Self::Perpendicular => "Perpendicular",
            Self::Intersection => "Intersection",
        }
    }
}

/// Mode for unsewn boundary splitting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SplitMode {
    #[default]
    Arclength,
    Ratio,
    Count,
}

impl SplitMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Arclength => "By Arclength (mm)",
            Self::Ratio => "By Ratio (0..1)",
            Self::Count => "By Count",
        }
    }
}

/// Pin display/creation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PinMode {
    #[default]
    Contour,
    Interior,
}

/// Pending cross-panel or cross-item measurement transition guard.
#[derive(Clone, Debug, PartialEq)]
pub enum MeasurementTransition {
    Select(String, PatternMeasurement),
    New(String),
    SwitchPiece(String),
}

/// Active measurement draft state.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasurementDraftState {
    pub value: PatternMeasurement,
    pub base: Option<PatternMeasurement>,
    pub is_new: bool,
}

impl MeasurementDraftState {
    pub fn new_draft(panel_id: &str, edge_a: &str, edge_b: &str) -> Self {
        Self {
            value: PatternMeasurement {
                id: format!("meas_{}", panel_id),
                label: "Width Check".into(),
                a: MeasurementAnchor {
                    edge: edge_a.into(),
                    t: 0.25,
                },
                b: MeasurementAnchor {
                    edge: edge_b.into(),
                    t: 0.75,
                },
            },
            base: None,
            is_new: true,
        }
    }

    pub fn is_valid_for(&self, polygon: &PolygonPattern) -> bool {
        if self.value.label.trim().is_empty() {
            return false;
        }
        polygon.measurement_length(&self.value).is_ok()
    }
}

/// Complete UI state for Property Editor Part 1.
#[derive(Clone, Debug)]
pub struct PropertiesPart1State {
    // Accordion open / collapsed flags
    pub information_open: bool,
    pub selected_line_open: bool,
    pub arrange_open: bool,
    pub measurements_open: bool,
    pub pins_open: bool,

    // Point & Dimensions
    pub selected_point: Option<usize>,
    pub point_dx_mm: f64,
    pub point_dy_mm: f64,
    pub rect_width_mm: f64,
    pub rect_height_mm: f64,

    // Construction
    pub construction_mode: ConstructionMode,
    pub construction_reference: Option<String>,
    pub construction_second_reference: Option<String>,
    pub construction_value_mm: f64,

    // Selected Line / Boundary
    pub selected_boundary: Option<BoundaryRef>,
    pub target_length_input_mm: f64,
    pub split_mode: SplitMode,
    pub split_value: f64,
    pub seam_allowance_input_mm: f64,
    pub new_notch_t: f64,
    pub new_notch_len_mm: f64,
    pub new_notch_kind: NotchKind,

    // 3D Arrangement
    pub placement_preview: Option<Placement>,
    pub group_pivot: DVec3,

    // Attached Ruler Measurements
    pub selected_measurement_id: Option<String>,
    pub measurement_draft: Option<MeasurementDraftState>,
    pub pending_transition: Option<MeasurementTransition>,

    // Persistent Mesh Pins
    pub pin_mode: PinMode,
    pub new_pin_u_mm: f64,
    pub new_pin_v_mm: f64,
}

impl Default for PropertiesPart1State {
    fn default() -> Self {
        Self {
            information_open: true,
            selected_line_open: true,
            arrange_open: true,
            measurements_open: true,
            pins_open: true,

            selected_point: None,
            point_dx_mm: 0.0,
            point_dy_mm: 0.0,
            rect_width_mm: 400.0,
            rect_height_mm: 500.0,

            construction_mode: ConstructionMode::Project,
            construction_reference: None,
            construction_second_reference: None,
            construction_value_mm: 50.0,

            selected_boundary: None,
            target_length_input_mm: 100.0,
            split_mode: SplitMode::Arclength,
            split_value: 50.0,
            seam_allowance_input_mm: 10.0,
            new_notch_t: 0.5,
            new_notch_len_mm: 5.0,
            new_notch_kind: NotchKind::V,

            placement_preview: None,
            group_pivot: DVec3::ZERO,

            selected_measurement_id: None,
            measurement_draft: None,
            pending_transition: None,

            pin_mode: PinMode::Contour,
            new_pin_u_mm: 100.0,
            new_pin_v_mm: 100.0,
        }
    }
}

/// Render the complete Property Editor Part 1 (5 accordions).
pub fn render_properties_part1(
    state: &PropertiesPart1State,
    panel: &Panel,
    template: &GarmentTemplate,
) -> Node {
    let polygon = panel.polygon.as_ref();
    ui! {
        <div id="property-editor-part1" class="property-editor">
            {render_information_accordion(state, panel, template, polygon)}
            {render_selected_line_accordion(state, panel, template, polygon)}
            {render_arrange_accordion(state, panel)}
            {render_measurements_accordion(state, panel, polygon)}
            {render_pins_accordion(state, panel)}
        </div>
    }
}

// ----------------------------------------------------------------------------
// 1. Piece & Point Information Accordion
// ----------------------------------------------------------------------------

pub fn render_information_accordion(
    state: &PropertiesPart1State,
    panel: &Panel,
    template: &GarmentTemplate,
    polygon: Option<&PolygonPattern>,
) -> Node {
    let open_class = if state.information_open {
        "accordion open"
    } else {
        "accordion collapsed"
    };

    // Layout and grainline
    let layout = panel.layout_2d.unwrap_or(DVec2::ZERO);
    let layout_x_mm = authoring::m_to_mm(layout.x);
    let layout_y_mm = authoring::m_to_mm(layout.y);
    let rot_deg = panel.layout_rotation_degrees;
    let grain_deg = panel.grain_degrees;

    // Linked mirror status
    let mirror_link = template
        .mirror_links
        .iter()
        .find(|l| l.a == panel.id || l.b == panel.id);
    let mirror_status = match mirror_link {
        Some(link) => {
            let partner = if link.a == panel.id { &link.b } else { &link.a };
            let axis_name = match link.axis {
                MirrorAxis::X => "X",
                MirrorAxis::Y => "Y",
            };
            format!("Linked: {} (Axis {})", partner, axis_name)
        }
        None => "Unlinked (Independent piece)".to_string(),
    };

    ui! {
        <section class={open_class} id="accordion-information">
            <header class="accordion-header">
                <h3 class="accordion-title">Information</h3>
            </header>
            <div class="accordion-body">
                <div class="property-row" id="piece-identity">
                    <span class="label">Piece ID</span>
                    <span class="value" id="piece-id">{panel.id.clone()}</span>
                </div>
                <div class="property-row">
                    <span class="label">Piece Name</span>
                    <span class="value" id="piece-name">{panel.name.clone()}</span>
                </div>
                <div class="property-row" id="board-layout">
                    <span class="label">2D Board Layout</span>
                    <span class="value" id="layout-2d-origin">
                        {format!("X: {:.1} mm, Y: {:.1} mm", layout_x_mm, layout_y_mm)}
                    </span>
                    <span class="value" id="layout-rotation">
                        {format!("{:.1}°", rot_deg)}
                    </span>
                </div>
                <div class="property-row" id="grainline">
                    <span class="label">Grainline</span>
                    <span class="value" id="grainline-degrees">{format!("{:.1}°", grain_deg)}</span>
                </div>
                <div class="property-row" id="mirror-partner">
                    <span class="label">Linked Mirror</span>
                    <span class="badge" id="linked-mirror-status">{mirror_status}</span>
                </div>

                {render_point_and_construction(state, panel, template, polygon)}
            </div>
        </section>
    }
}

fn render_point_and_construction(
    state: &PropertiesPart1State,
    panel: &Panel,
    template: &GarmentTemplate,
    polygon: Option<&PolygonPattern>,
) -> Node {
    let poly = match polygon {
        Some(p) => p,
        None => {
            return ui! {
                <div class="point-section empty">
                    <p class="hint">No polygonal rest geometry</p>
                </div>
            };
        }
    };

    let point_row = if let Some(k) = state.selected_point.filter(|&k| k < poly.points.len()) {
        let pt = poly.points[k];
        let pt_x_mm = authoring::m_to_mm(pt.x);
        let pt_y_mm = authoring::m_to_mm(pt.y);
        ui! {
            <div class="point-movement-card" id="selected-point-controls">
                <div class="section-subtitle">{format!("Point {} (mm)", k + 1)}</div>
                <div class="field-row">
                    <label>Position</label>
                    <span class="value" id="point-coords">{format!("X: {:.2} mm, Y: {:.2} mm", pt_x_mm, pt_y_mm)}</span>
                </div>
                <div class="field-row">
                    <label>Offset dx / dy</label>
                    <span class="value" id="point-delta">{format!("{:.1} mm / {:.1} mm", state.point_dx_mm, state.point_dy_mm)}</span>
                    <button class="btn btn-nudge" id="nudge-minus-10">-10</button>
                    <button class="btn btn-nudge" id="nudge-plus-10">+10</button>
                </div>
                <div class="field-row">
                    <label>Piece Dimensions</label>
                    <span class="value" id="piece-dimensions">{format!("{:.1} × {:.1} mm", state.rect_width_mm, state.rect_height_mm)}</span>
                </div>
            </div>
        }
    } else {
        ui! {
            <div class="point-movement-card empty" id="no-point-selected">
                <p class="hint">Click a point on the 2D board to inspect its exact position</p>
            </div>
        }
    };

    // Construction readout
    let construction_row = render_numeric_construction(state, panel, template, poly);

    ui! {
        <div class="point-and-construction-container">
            {point_row}
            {construction_row}
        </div>
    }
}

fn render_numeric_construction(
    state: &PropertiesPart1State,
    panel: &Panel,
    template: &GarmentTemplate,
    poly: &PolygonPattern,
) -> Node {
    let target_edge = state
        .selected_point
        .and_then(|k| poly.edges.get(k))
        .cloned()
        .unwrap_or_else(|| poly.edges.first().cloned().unwrap_or_default());

    let target_ref = ConstructionRef {
        panel: panel.id.clone(),
        edge: target_edge.clone(),
    };

    let ref_edge = state
        .construction_reference
        .clone()
        .unwrap_or_else(|| poly.edges.last().cloned().unwrap_or_default());
    let reference_ref = ConstructionRef {
        panel: panel.id.clone(),
        edge: ref_edge.clone(),
    };

    let op = match state.construction_mode {
        ConstructionMode::Project => PointConstruction::Project,
        ConstructionMode::Distance => {
            PointConstruction::Distance(authoring::mm_to_m(state.construction_value_mm))
        }
        ConstructionMode::Perpendicular => {
            PointConstruction::Perpendicular(authoring::mm_to_m(state.construction_value_mm))
        }
        ConstructionMode::Intersection => {
            let second_edge = state
                .construction_second_reference
                .clone()
                .unwrap_or_else(|| poly.edges.first().cloned().unwrap_or_default());
            PointConstruction::Intersection(ConstructionRef {
                panel: panel.id.clone(),
                edge: second_edge,
            })
        }
    };

    let preview_result = authoring::constructed_point(template, &target_ref, &reference_ref, op);

    let (preview_text, preview_class, apply_enabled) = match preview_result {
        Ok(p) => (
            format!(
                "Local result: X {:.2} mm · Y {:.2} mm",
                authoring::m_to_mm(p.x),
                authoring::m_to_mm(p.y)
            ),
            "preview-ok",
            true,
        ),
        Err(err) => (err, "preview-error", false),
    };

    let apply_btn_class = if apply_enabled {
        "btn btn-primary"
    } else {
        "btn btn-disabled"
    };

    ui! {
        <div class="construction-card" id="numeric-construction-panel">
            <h4 class="card-title">Numeric Point Construction</h4>
            <div class="mode-tabs" id="construction-modes">
                <button class="btn tab-btn" id="mode-project">Project</button>
                <button class="btn tab-btn" id="mode-distance">Distance</button>
                <button class="btn tab-btn" id="mode-perpendicular">Perpendicular</button>
            </div>
            <div class="field-row">
                <label>Reference Edge</label>
                <span class="value" id="construction-reference-edge">{ref_edge}</span>
            </div>
            <div class="field-row">
                <label>Value (mm)</label>
                <span class="value" id="construction-val">{format!("{:.1} mm", state.construction_value_mm)}</span>
            </div>
            <div class={preview_class} id="construction-preview-readout">
                {preview_text}
            </div>
            <div class="action-row">
                <button class={apply_btn_class} id="btn-apply-construction">
                    Apply Construction
                </button>
            </div>
        </div>
    }
}

// ----------------------------------------------------------------------------
// 2. Selected Boundary / Line Accordion
// ----------------------------------------------------------------------------

pub fn render_selected_line_accordion(
    state: &PropertiesPart1State,
    panel: &Panel,
    template: &GarmentTemplate,
    polygon: Option<&PolygonPattern>,
) -> Node {
    let open_class = if state.selected_line_open {
        "accordion open"
    } else {
        "accordion collapsed"
    };
    let poly = match polygon {
        Some(p) => p,
        None => {
            return ui! {
                <section class={open_class} id="accordion-selected-line">
                    <header class="accordion-header"><h3 class="accordion-title">Selected Line</h3></header>
                    <div class="accordion-body"><p class="hint">No polygonal boundary</p></div>
                </section>
            };
        }
    };

    let boundary_edge = state
        .selected_boundary
        .as_ref()
        .and_then(|b| b.edge.as_deref())
        .map(str::to_string)
        .or_else(|| poly.edges.first().cloned())
        .unwrap_or_default();

    let edge_idx = poly.edges.iter().position(|e| e == &boundary_edge);
    let arclength_m = edge_idx
        .and_then(|idx| poly.edge_points(idx).ok())
        .map(|pts| polyline_length(&pts))
        .unwrap_or(0.0);
    let arclength_mm = authoring::m_to_mm(arclength_m);

    // Target length override
    let stored_target_m = poly.target_length(&boundary_edge);
    let (target_text, dev_text) = match stored_target_m {
        Some(target) => {
            let target_mm = authoring::m_to_mm(target);
            let dev_mm = authoring::deviation_mm(arclength_m, target);
            (format!("{:.1} mm", target_mm), format!("{:+.2} mm", dev_mm))
        }
        None => ("None".to_string(), "0.00 mm".to_string()),
    };

    // Seam allowance
    let allowance_mm = authoring::m_to_mm(poly.allowance_of(&boundary_edge));
    let clear_allowance_class = if poly.seam_allowances.contains_key(&boundary_edge) {
        "btn btn-outline"
    } else {
        "btn btn-disabled"
    };

    // Synchronized seam cutouts & linked-mirror badge (EXO-CAD-013 / FC-0002)
    let is_sewn = template.seams.iter().any(|s| {
        (s.a.panel == panel.id && s.a.edge.as_deref() == Some(boundary_edge.as_str()))
            || (s.b.panel == panel.id && s.b.edge.as_deref() == Some(boundary_edge.as_str()))
    });
    let is_linked_mirror = template
        .mirror_links
        .iter()
        .any(|l| l.a == panel.id || l.b == panel.id);

    let badge_text = if is_sewn && is_linked_mirror {
        "Sewn · Linked-Mirror Cutout Synchronized (EXO-CAD-013)"
    } else if is_sewn {
        "Sewn Boundary · Cutout Synchronized Ready (EXO-CAD-013)"
    } else if is_linked_mirror {
        "Linked Mirror Boundary"
    } else {
        "Unsewn Free Boundary"
    };

    // Notches list
    let notches_nodes = render_notches_list(poly, &boundary_edge);

    ui! {
        <section class={open_class} id="accordion-selected-line">
            <header class="accordion-header">
                <h3 class="accordion-title">Selected Line</h3>
            </header>
            <div class="accordion-body">
                <div class="property-row" id="boundary-ref">
                    <span class="label">Boundary Ref</span>
                    <span class="value" id="boundary-panel-edge">{format!("{} · {}", panel.id, boundary_edge)}</span>
                </div>
                <div class="property-row" id="rest-arclength">
                    <span class="label">Rest Arclength</span>
                    <span class="value" id="arclength-value">{format!("{:.1} mm", arclength_mm)}</span>
                </div>

                <div class="badge-row">
                    <span class="badge badge-info" id="exo-cutout-badge">{badge_text}</span>
                </div>

                // Target length override
                <div class="section-group" id="target-length-controls">
                    <div class="group-title">Target Length Override</div>
                    <div class="field-row">
                        <label>Target</label>
                        <span class="value" id="target-length-display">{target_text}</span>
                        <span class="value deviation" id="target-deviation">{format!("Deviation: {}", dev_text)}</span>
                    </div>
                    <div class="action-row">
                        <button class="btn btn-secondary" id="btn-apply-target">Apply Target</button>
                        <button class="btn btn-outline" id="btn-clear-target">Clear Target</button>
                    </div>
                </div>

                // Unsewn split controls
                <div class="section-group" id="split-controls">
                    <div class="group-title">Unsewn Boundary Split</div>
                    <div class="split-modes" id="split-mode-selector">
                        <span class="mode-label">{state.split_mode.label()}</span>
                    </div>
                    <div class="field-row">
                        <label>Split Value</label>
                        <span class="value" id="split-val-display">{format!("{:.1}", state.split_value)}</span>
                    </div>
                    <button class="btn btn-secondary" id="btn-split-boundary">Split Boundary</button>
                </div>

                // Seam allowance width
                <div class="section-group" id="seam-allowance-controls">
                    <div class="group-title">Seam Allowance</div>
                    <div class="field-row">
                        <label>Allowance Width</label>
                        <span class="value" id="seam-allowance-val">{format!("{:.1} mm", allowance_mm)}</span>
                    </div>
                    <div class="action-row">
                        <button class="btn btn-secondary" id="btn-apply-allowance">Apply Allowance</button>
                        <button class={clear_allowance_class} id="btn-clear-allowance">Clear Allowance</button>
                    </div>
                </div>

                // Sewing-line notches
                <div class="section-group" id="notches-controls">
                    <div class="group-title">Sewing-Line Notches</div>
                    <div class="field-row">
                        <label>Add at t (0..1)</label>
                        <span class="value" id="new-notch-t-val">{format!("{:.2}", state.new_notch_t)}</span>
                        <label>Len (mm)</label>
                        <span class="value" id="new-notch-len-val">{format!("{:.1} mm", state.new_notch_len_mm)}</span>
                    </div>
                    <button class="btn btn-primary" id="btn-add-notch">Add Notch</button>
                    <div class="notches-list" id="notches-list-items">
                        {notches_nodes}
                    </div>
                </div>
            </div>
        </section>
    }
}

fn render_notches_list(poly: &PolygonPattern, edge: &str) -> Vec<Node> {
    match poly.notches.get(edge) {
        Some(list) if !list.is_empty() => list
            .iter()
            .enumerate()
            .map(|(i, notch)| {
                let kind_name = match notch.kind {
                    NotchKind::V => "V",
                    NotchKind::Slit => "Slit",
                };
                let len_mm = authoring::m_to_mm(notch.length_m);
                ui! {
                    <div class="notch-row" id={format!("notch-row-{}", i)}>
                        <span class="notch-info">
                            {format!("{}: t={:.2} · {} · {:.1} mm", i + 1, notch.at, kind_name, len_mm)}
                        </span>
                        <button class="btn btn-danger btn-sm" id={format!("btn-delete-notch-{}", i)}>Delete</button>
                    </div>
                }
            })
            .collect(),
        _ => vec![ui! {
            <p class="hint empty-notches" id="no-notches-hint">No notches placed on this boundary</p>
        }],
    }
}

// ----------------------------------------------------------------------------
// 3. 3D Arrangement Accordion
// ----------------------------------------------------------------------------

pub fn render_arrange_accordion(state: &PropertiesPart1State, panel: &Panel) -> Node {
    let open_class = if state.arrange_open {
        "accordion open"
    } else {
        "accordion collapsed"
    };
    let placement = state.placement_preview.as_ref().unwrap_or(&panel.placement);

    let t = placement.translation;
    let r = placement.rotation_degrees;

    // Group transform pivot
    let pivot = state.group_pivot;
    let pivot_text = format!(
        "Pivot: [X: {:.1} mm, Y: {:.1} mm, Z: {:.1} mm]",
        authoring::m_to_mm(pivot.x),
        authoring::m_to_mm(pivot.y),
        authoring::m_to_mm(pivot.z),
    );

    let has_preview = state.placement_preview.is_some();
    let commit_class = if has_preview {
        "btn btn-primary"
    } else {
        "btn btn-disabled"
    };

    ui! {
        <section class={open_class} id="accordion-arrange">
            <header class="accordion-header">
                <h3 class="accordion-title">Arrange</h3>
            </header>
            <div class="accordion-body">
                <div class="section-group" id="arrange-3d-translation">
                    <div class="group-title">3D Translation (m)</div>
                    <div class="field-row">
                        <span class="coord" id="trans-x">{format!("X: {:.3} m", t.x)}</span>
                        <span class="coord" id="trans-y">{format!("Y: {:.3} m", t.y)}</span>
                        <span class="coord" id="trans-z">{format!("Z: {:.3} m", t.z)}</span>
                    </div>
                </div>

                <div class="section-group" id="arrange-3d-rotation">
                    <div class="group-title">Euler Rotation (deg)</div>
                    <div class="field-row">
                        <span class="coord" id="rot-x">{format!("RX: {:.1}°", r.x)}</span>
                        <span class="coord" id="rot-y">{format!("RY: {:.1}°", r.y)}</span>
                        <span class="coord" id="rot-z">{format!("RZ: {:.1}°", r.z)}</span>
                    </div>
                </div>

                <div class="pivot-readout" id="group-transform-pivot">
                    <span class="pivot-value">{pivot_text}</span>
                </div>

                <div class="action-row" id="arrange-actions">
                    <button class={commit_class} id="btn-commit-placement">Commit Placement</button>
                    <button class="btn btn-outline" id="btn-cancel-preview">Cancel Preview</button>
                </div>

                <div class="invariant-note" id="arrange-physics-invariant">
                    <p class="hint">Placement preview is not new rest geometry; 2D coordinates define material rest geometry.</p>
                </div>
            </div>
        </section>
    }
}

// ----------------------------------------------------------------------------
// 4. Attached Ruler Measurements Accordion (FC-0005)
// ----------------------------------------------------------------------------

pub fn render_measurements_accordion(
    state: &PropertiesPart1State,
    panel: &Panel,
    polygon: Option<&PolygonPattern>,
) -> Node {
    let open_class = if state.measurements_open {
        "accordion open"
    } else {
        "accordion collapsed"
    };
    let poly = match polygon {
        Some(p) => p,
        None => {
            return ui! {
                <section class={open_class} id="accordion-measurements">
                    <header class="accordion-header"><h3 class="accordion-title">Measurements</h3></header>
                    <div class="accordion-body"><p class="hint">Measurements require an editable polygonal piece.</p></div>
                </section>
            };
        }
    };

    // Pending cross-panel transition guard banner
    if let Some(transition) = &state.pending_transition {
        return render_pending_transition_view(
            open_class,
            transition,
            state.measurement_draft.as_ref(),
            poly,
        );
    }

    // Normal measurements view
    let saved_rows = render_saved_measurements_rows(poly, state.selected_measurement_id.as_deref());
    let draft_card = render_active_measurement_draft(panel, poly, state.measurement_draft.as_ref());

    ui! {
        <section class={open_class} id="accordion-measurements">
            <header class="accordion-header">
                <h3 class="accordion-title">Measurements</h3>
            </header>
            <div class="accordion-body">
                <p class="hint">Straight-line distances in flat material coordinates. Click two boundaries to measure.</p>

                <div class="saved-measurements-list" id="saved-measurements-container">
                    {saved_rows}
                </div>

                <div class="action-row">
                    <button class="btn btn-secondary" id="btn-new-measurement">New Measurement</button>
                </div>

                {draft_card}
            </div>
        </section>
    }
}

fn render_pending_transition_view(
    open_class: &str,
    _transition: &MeasurementTransition,
    draft: Option<&MeasurementDraftState>,
    poly: &PolygonPattern,
) -> Node {
    let draft_is_valid = draft.is_some_and(|d| d.is_valid_for(poly));
    let save_btn_class = if draft_is_valid {
        "btn btn-primary"
    } else {
        "btn btn-disabled"
    };

    ui! {
        <section class={open_class} id="accordion-measurements">
            <header class="accordion-header">
                <h3 class="accordion-title">Measurements</h3>
            </header>
            <div class="accordion-body">
                <div class="warning-banner" id="measurement-transition-banner">
                    <p class="warning-title">Save the current measurement draft before continuing?</p>
                    <div class="action-row">
                        <button class={save_btn_class} id="btn-save-draft-continue">Save draft and continue</button>
                        <button class="btn btn-secondary" id="btn-discard-draft-continue">Discard draft and continue</button>
                        <button class="btn btn-outline" id="btn-cancel-switching">Cancel switching measurement</button>
                    </div>
                </div>
            </div>
        </section>
    }
}

fn render_saved_measurements_rows(poly: &PolygonPattern, selected_id: Option<&str>) -> Vec<Node> {
    if poly.measurements.is_empty() {
        return vec![ui! {
            <p class="hint empty" id="no-saved-measurements">No saved measurements on this piece</p>
        }];
    }

    poly.measurements
        .iter()
        .map(|m| {
            let length_m = poly.measurement_length(m).unwrap_or(0.0);
            let length_mm = authoring::m_to_mm(length_m);
            let is_sel = selected_id == Some(m.id.as_str());
            let row_class = if is_sel { "measurement-row selected" } else { "measurement-row" };

            ui! {
                <div class={row_class} id={format!("measurement-row-{}", m.id)}>
                    <span class="measurement-label">{m.label.clone()}</span>
                    <span class="measurement-length">{format!("{:.2} mm", length_mm)}</span>
                    <button class="btn btn-sm" id={format!("btn-select-meas-{}", m.id)}>Select</button>
                </div>
            }
        })
        .collect()
}

fn render_active_measurement_draft(
    _panel: &Panel,
    poly: &PolygonPattern,
    draft: Option<&MeasurementDraftState>,
) -> Option<Node> {
    let d = draft?;
    let meas = &d.value;
    let length_result = poly.measurement_length(meas);

    let (len_text, len_class, valid) = match length_result {
        Ok(l) => (
            format!("{:.2} mm", authoring::m_to_mm(l)),
            "length-valid",
            true,
        ),
        Err(err) => (err, "length-error", false),
    };

    let has_changed = d.base.as_ref() != Some(meas);
    let apply_enabled = valid && has_changed && !meas.label.trim().is_empty();
    let apply_btn_class = if apply_enabled {
        "btn btn-primary"
    } else {
        "btn btn-disabled"
    };
    let apply_label = if d.base.is_some() {
        "Apply measurement edit"
    } else {
        "Save measurement"
    };

    let delete_btn = if d.base.is_some() {
        Some(ui! {
            <button class="btn btn-danger" id="btn-delete-measurement">Delete measurement</button>
        })
    } else {
        None
    };

    Some(ui! {
        <div class="active-measurement-card" id="measurement-draft-card">
            <h4 class="card-title">Active Measurement Draft</h4>
            <div class="field-row">
                <label>Label</label>
                <span class="value" id="meas-draft-label">{meas.label.clone()}</span>
            </div>
            <div class="field-row" id="meas-station-a">
                <label>Station A</label>
                <span class="value">{format!("{} · t={:.3}", meas.a.edge, meas.a.t)}</span>
            </div>
            <div class="field-row" id="meas-station-b">
                <label>Station B</label>
                <span class="value">{format!("{} · t={:.3}", meas.b.edge, meas.b.t)}</span>
            </div>
            <div class={len_class} id="meas-computed-distance">
                {len_text}
            </div>
            <div class="action-row">
                <button class={apply_btn_class} id="btn-apply-measurement">{apply_label}</button>
                {delete_btn}
                <button class="btn btn-outline" id="btn-cancel-measurement-draft">Cancel measurement draft</button>
            </div>
        </div>
    })
}

// ----------------------------------------------------------------------------
// 5. Persistent Mesh Pins Accordion (FC-0007)
// ----------------------------------------------------------------------------

pub fn render_pins_accordion(state: &PropertiesPart1State, panel: &Panel) -> Node {
    let open_class = if state.pins_open {
        "accordion open"
    } else {
        "accordion collapsed"
    };

    let mode_tabs = ui! {
        <div class="mode-tabs" id="pin-mode-selector">
            <button class="btn tab-btn active" id="pin-mode-contour">Contour Pins</button>
            <button class="btn tab-btn" id="pin-mode-interior">Interior Pins</button>
        </div>
    };

    let pins_list_nodes = match state.pin_mode {
        PinMode::Contour => render_contour_pins_list(panel),
        PinMode::Interior => render_interior_pins_list(panel),
    };

    ui! {
        <section class={open_class} id="accordion-pins">
            <header class="accordion-header">
                <h3 class="accordion-title">Pins</h3>
            </header>
            <div class="accordion-body">
                {mode_tabs}

                <div class="pins-list" id="pins-list-container">
                    {pins_list_nodes}
                </div>

                <div class="section-group" id="add-pin-controls">
                    <div class="group-title">Add Material Position Pin</div>
                    <div class="field-row">
                        <label>U (mm)</label>
                        <span class="value" id="new-pin-u">{format!("{:.1} mm", state.new_pin_u_mm)}</span>
                        <label>V (mm)</label>
                        <span class="value" id="new-pin-v">{format!("{:.1} mm", state.new_pin_v_mm)}</span>
                    </div>
                    <button class="btn btn-primary" id="btn-add-pin">Add Pin</button>
                </div>

                <div class="action-row">
                    <button class="btn btn-danger btn-outline" id="btn-clear-piece-pins">Clear Piece Pins</button>
                </div>
            </div>
        </section>
    }
}

fn render_contour_pins_list(panel: &Panel) -> Vec<Node> {
    if panel.pins.is_empty() {
        return vec![ui! {
            <p class="hint empty" id="no-contour-pins">No contour pins on this piece</p>
        }];
    }

    panel
        .pins
        .iter()
        .enumerate()
        .map(|(i, corner)| {
            let name = match corner {
                Corner::TopLeft => "Top Left",
                Corner::TopRight => "Top Right",
                Corner::BottomRight => "Bottom Right",
                Corner::BottomLeft => "Bottom Left",
            };
            ui! {
                <div class="pin-row" id={format!("contour-pin-{}", i)}>
                    <span class="pin-info">{format!("Contour: {}", name)}</span>
                    <button class="btn btn-sm btn-danger" id={format!("btn-remove-contour-pin-{}", i)}>Remove Pin</button>
                </div>
            }
        })
        .collect()
}

fn render_interior_pins_list(panel: &Panel) -> Vec<Node> {
    if panel.pin_points.is_empty() {
        return vec![ui! {
            <p class="hint empty" id="no-interior-pins">No interior pins on this piece</p>
        }];
    }

    panel
        .pin_points
        .iter()
        .enumerate()
        .map(|(i, uv)| {
            // RENDER IN MM WITHOUT EXPOSING TRANSIENT MESH VERTEX INDICES!
            let u_mm = authoring::m_to_mm(uv.x);
            let v_mm = authoring::m_to_mm(uv.y);
            ui! {
                <div class="pin-row" id={format!("interior-pin-{}", i)}>
                    <span class="pin-info">{format!("X {:.2} mm · Y {:.2} mm", u_mm, v_mm)}</span>
                    <button class="btn btn-sm btn-danger" id={format!("btn-remove-interior-pin-{}", i)}>Remove Pin</button>
                </div>
            }
        })
        .collect()
}

// ----------------------------------------------------------------------------
// DOM inspection test helpers
// ----------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    pub fn find_by_id<'a>(node: &'a Node, target_id: &str) -> Option<&'a ElementNode> {
        match node {
            Node::Element(el) => {
                if el.id.as_deref() == Some(target_id) {
                    return Some(el);
                }
                for child in &el.children {
                    if let Some(found) = find_by_id(child, target_id) {
                        return Some(found);
                    }
                }
                None
            }
            Node::Text(_) => None,
        }
    }

    #[allow(dead_code)]
    pub fn find_all_by_class<'a>(node: &'a Node, target_class: &str) -> Vec<&'a ElementNode> {
        let mut results = Vec::new();
        collect_by_class(node, target_class, &mut results);
        results
    }

    #[allow(dead_code)]
    fn collect_by_class<'a>(
        node: &'a Node,
        target_class: &str,
        results: &mut Vec<&'a ElementNode>,
    ) {
        if let Node::Element(el) = node {
            if el.classes.iter().any(|c| c == target_class) {
                results.push(el);
            }
            for child in &el.children {
                collect_by_class(child, target_class, results);
            }
        }
    }

    pub fn collect_text(node: &Node) -> String {
        let mut text = String::new();
        append_text(node, &mut text);
        text
    }

    fn append_text(node: &Node, out: &mut String) {
        match node {
            Node::Text(t) => {
                out.push_str(t);
                out.push(' ');
            }
            Node::Element(el) => {
                for child in &el.children {
                    append_text(child, out);
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Unit Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::test_helpers::*;
    use super::*;
    use crate::garment::{markings::Notch, seam::Side, template::MirrorLink};
    use crate::model::Pattern;

    fn test_polygon(width: f64, height: f64) -> PolygonPattern {
        PolygonPattern {
            points: vec![
                DVec2::ZERO,
                DVec2::new(width, 0.),
                DVec2::new(width, height),
                DVec2::new(0., height),
            ],
            edges: ["edge_0", "edge_1", "edge_2", "edge_3"]
                .map(String::from)
                .to_vec(),
            subdivisions: 1,
            meshing: None,
            curves: Default::default(),
            holes: vec![],
            lines: vec![],
            target_lengths: Default::default(),
            seam_allowances: Default::default(),
            notches: Default::default(),
            measurements: Default::default(),
        }
    }

    fn sample_test_fixture() -> (GarmentTemplate, Panel) {
        let mut template = GarmentTemplate::two_panel_demo();
        let mut panel = template.panels[0].clone();

        // Polygon setup
        let poly = test_polygon(0.4, 0.5);
        panel.polygon = Some(poly);
        panel.pattern = Pattern {
            width: 0.4,
            height: 0.5,
            nx: 4,
            ny: 5,
        };
        panel.layout_2d = Some(DVec2::new(0.12, 0.34));
        panel.layout_rotation_degrees = 45.0;
        panel.grain_degrees = 90.0;
        panel.pins = vec![Corner::TopLeft, Corner::TopRight];
        panel.pin_points = vec![DVec2::new(0.05, 0.05), DVec2::new(0.15, 0.25)];

        // Add mirror link in template
        template.mirror_links.push(MirrorLink {
            a: panel.id.clone(),
            b: "partner_piece".into(),
            axis: MirrorAxis::X,
        });

        // Update template so target/reference lookups find the panel polygon
        template.panels[0] = panel.clone();

        (template, panel)
    }

    #[test]
    fn test_rendering_five_accordions_from_real_domain_structs() {
        let (mut template, mut panel) = sample_test_fixture();

        // Add a seam using edge_0
        template.seams.push(crate::garment::seam::SeamSpec {
            id: "seam_edge0".into(),
            a: BoundaryRef {
                reversed: false,
                panel: panel.id.clone(),
                side: Side::Top,
                edge: Some("edge_0".into()),
                range: [0.0, 1.0],
            },
            b: BoundaryRef {
                reversed: true,
                panel: "partner_piece".into(),
                side: Side::Bottom,
                edge: Some("edge_2".into()),
                range: [0.0, 1.0],
            },
            a_tail: vec![],
            b_tail: vec![],
            reverse_b: false,
            notches: vec![],
            stiffness: 1000.0,
            allow_ease: false,
        });

        // Add notches and seam allowance
        if let Some(poly) = panel.polygon.as_mut() {
            poly.seam_allowances.insert("edge_0".into(), 0.015);
            poly.notches
                .insert("edge_0".into(), vec![Notch::new(0.5, 0.005, NotchKind::V)]);
            poly.measurements.push(PatternMeasurement {
                id: "meas_1".into(),
                label: "Chest Width".into(),
                a: MeasurementAnchor {
                    edge: "edge_0".into(),
                    t: 0.2,
                },
                b: MeasurementAnchor {
                    edge: "edge_2".into(),
                    t: 0.8,
                },
            });
        }
        template.panels[0] = panel.clone();

        let state = PropertiesPart1State {
            selected_point: Some(0),
            selected_boundary: Some(BoundaryRef {
                reversed: false,
                panel: panel.id.clone(),
                side: Side::Top,
                edge: Some("edge_0".into()),
                range: [0.0, 1.0],
            }),
            ..Default::default()
        };

        let node = render_properties_part1(&state, &panel, &template);

        // Verify root container
        let root = find_by_id(&node, "property-editor-part1").expect("Root container must exist");
        assert_eq!(root.tag, "div");

        // Verify each of the 5 accordions exists
        assert!(find_by_id(&node, "accordion-information").is_some());
        assert!(find_by_id(&node, "accordion-selected-line").is_some());
        assert!(find_by_id(&node, "accordion-arrange").is_some());
        assert!(find_by_id(&node, "accordion-measurements").is_some());
        assert!(find_by_id(&node, "accordion-pins").is_some());

        // Verify Information content
        let text = collect_text(&node);
        assert!(text.contains(&panel.id));
        assert!(text.contains("120.0 mm, Y: 340.0 mm")); // layout_2d mm
        assert!(text.contains("45.0°")); // layout_rotation_degrees
        assert!(text.contains("90.0°")); // grainline
        assert!(text.contains("Linked: partner_piece (Axis X)")); // mirror partner
        assert!(text.contains("Chest Width")); // measurement label
        assert!(text.contains("Sewn · Linked-Mirror Cutout Synchronized (EXO-CAD-013)")); // EXO-CAD-013 badge
    }

    #[test]
    fn test_pending_measurement_draft_transition_banner_invariants() {
        let (_template, panel) = sample_test_fixture();
        let mut state = PropertiesPart1State::default();

        // 1. With a VALID draft pending transition
        let poly = panel.polygon.as_ref().unwrap();
        let valid_draft = MeasurementDraftState {
            value: PatternMeasurement {
                id: "meas_valid".into(),
                label: "Valid Meas".into(),
                a: MeasurementAnchor {
                    edge: "edge_0".into(),
                    t: 0.1,
                },
                b: MeasurementAnchor {
                    edge: "edge_2".into(),
                    t: 0.9,
                },
            },
            base: None,
            is_new: true,
        };
        assert!(valid_draft.is_valid_for(poly));

        state.measurement_draft = Some(valid_draft);
        state.pending_transition = Some(MeasurementTransition::SwitchPiece("other_panel".into()));

        let node = render_measurements_accordion(&state, &panel, Some(poly));

        // Must show transition banner
        assert!(find_by_id(&node, "measurement-transition-banner").is_some());

        // Save button must be enabled
        let save_btn = find_by_id(&node, "btn-save-draft-continue").expect("Save button exists");
        assert!(save_btn.classes.iter().any(|c| c == "btn-primary"));
        assert!(!save_btn.classes.iter().any(|c| c == "btn-disabled"));

        // Discard and Cancel buttons exist
        assert!(find_by_id(&node, "btn-discard-draft-continue").is_some());
        assert!(find_by_id(&node, "btn-cancel-switching").is_some());

        // 2. With an INVALID draft (empty label)
        state.measurement_draft.as_mut().unwrap().value.label = "".into();
        let invalid_node = render_measurements_accordion(&state, &panel, Some(poly));
        let save_btn_invalid =
            find_by_id(&invalid_node, "btn-save-draft-continue").expect("Save button exists");
        assert!(save_btn_invalid.classes.iter().any(|c| c == "btn-disabled"));
    }

    #[test]
    fn test_interior_and_contour_pins_render_coordinates_in_mm_without_mesh_vertex_indices() {
        let (_template, mut panel) = sample_test_fixture();
        panel.pin_points = vec![
            DVec2::new(0.0425, 0.0812), // 42.50 mm, 81.20 mm
            DVec2::new(0.1234, 0.2345), // 123.40 mm, 234.50 mm
        ];

        let state = PropertiesPart1State {
            pin_mode: PinMode::Interior,
            ..Default::default()
        };

        let node = render_pins_accordion(&state, &panel);

        // Check text readout
        let text = collect_text(&node);
        assert!(text.contains("X 42.50 mm · Y 81.20 mm"));
        assert!(text.contains("X 123.40 mm · Y 234.50 mm"));

        // Invariant check: never expose vertex index keywords like "Vertex #" or "v_idx"
        assert!(!text.contains("Vertex"));
        assert!(!text.contains("vertex_index"));

        // Now check Contour pins
        let state_contour = PropertiesPart1State {
            pin_mode: PinMode::Contour,
            ..Default::default()
        };
        let contour_node = render_pins_accordion(&state_contour, &panel);
        let contour_text = collect_text(&contour_node);
        assert!(contour_text.contains("Top Left"));
        assert!(contour_text.contains("Top Right"));
    }

    #[test]
    fn test_arrange_accordion_pivot_and_physics_invariant_note() {
        let (_template, panel) = sample_test_fixture();
        let state = PropertiesPart1State {
            group_pivot: DVec3::new(0.2, -0.25, 0.0),
            placement_preview: Some(Placement {
                translation: DVec3::new(1.0, 2.0, 3.0),
                rotation_degrees: DVec3::new(10.0, 20.0, 30.0),
            }),
            ..Default::default()
        };

        let node = render_arrange_accordion(&state, &panel);
        let text = collect_text(&node);

        // Pivot in mm
        assert!(text.contains("Pivot: [X: 200.0 mm, Y: -250.0 mm, Z: 0.0 mm]"));

        // Translation and rotation
        assert!(text.contains("X: 1.000 m"));
        assert!(text.contains("RX: 10.0°"));

        // Invariant statement
        assert!(text.contains("Placement preview is not new rest geometry; 2D coordinates define material rest geometry."));

        // Buttons exist
        let commit_btn = find_by_id(&node, "btn-commit-placement").expect("Commit placement btn");
        assert!(commit_btn.classes.iter().any(|c| c == "btn-primary"));
        assert!(find_by_id(&node, "btn-cancel-preview").is_some());
    }

    #[test]
    fn test_numeric_point_construction_preview_and_apply_state() {
        let (template, panel) = sample_test_fixture();
        let state = PropertiesPart1State {
            selected_point: Some(0),
            construction_mode: ConstructionMode::Distance,
            construction_reference: Some("edge_1".into()),
            construction_value_mm: 100.0,
            ..Default::default()
        };

        let poly = panel.polygon.as_ref().unwrap();
        let node = render_numeric_construction(&state, &panel, &template, poly);
        let text = collect_text(&node);

        assert!(text.contains("Numeric Point Construction"));
        assert!(text.contains("Local result: X"));
        assert!(text.contains("Apply Construction"));

        let apply_btn = find_by_id(&node, "btn-apply-construction").expect("Apply button");
        assert!(apply_btn.classes.iter().any(|c| c == "btn-primary"));
    }
}

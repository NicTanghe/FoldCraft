//! Comprehensive CPU tests for CAD cut-and-sewn darts, seam allowance guides, and notches.
//!
//! Follows FoldCraft invariants:
//! - Rest coordinates in meters define rest geometry.
//! - Editable files reference panel/boundary IDs and material coordinates, never persistent vertex indices.
//! - Independent property tests and unit tests.

use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command, History},
    garment::{
        GarmentTemplate,
        darts::DartSpec,
        export,
        markings::{Notch, NotchKind},
        meshing::MeshingSettings,
        polygon::PolygonPattern,
        presets::{self, GarmentKind},
    },
};

fn rectangle_template(width: f64, height: f64) -> GarmentTemplate {
    authoring::apply(
        &presets::basic(GarmentKind::Tshirt, 0),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(width, 0.),
                DVec2::new(width, height),
                DVec2::new(0., height),
            ],
            tangents: vec![],
            meshing: MeshingSettings::default(),
            fabric: "woven-reference".into(),
            replace: true,
        },
    )
    .unwrap()
    .template
}

fn plain_rectangle(width: f64, height: f64) -> PolygonPattern {
    PolygonPattern {
        points: vec![
            DVec2::ZERO,
            DVec2::new(width, 0.),
            DVec2::new(width, height),
            DVec2::new(0., height),
        ],
        edges: vec!["bottom".into(), "right".into(), "top".into(), "left".into()],
        subdivisions: 0,
        meshing: None,
        curves: Default::default(),
        holes: vec![],
        lines: vec![],
        target_lengths: Default::default(),
        seam_allowances: Default::default(),
        notches: Default::default(),
        measurements: vec![],
    }
}

// =========================================================================
// SECTION 1: CUT-AND-SEWN DARTS
// =========================================================================

#[test]
fn dart_intake_apex_specification_from_manufacturing_parameters() {
    let poly = plain_rectangle(0.3, 0.2);
    // Center at 40% along "bottom" edge (starts at 0.0, ends at 0.3 -> center at x = 0.12).
    // Intake = 30 mm (0.03 m), depth = 80 mm (0.08 m).
    let spec = DartSpec::from_intake(&poly, "bottom", 0.4, 0.03, 0.08).unwrap();
    assert_eq!(spec.edge, "bottom");
    // Edge length = 0.3 m. Half intake = 0.015 m = 0.05 normalized t.
    // Left = 0.4 - 0.05 = 0.35; Right = 0.4 + 0.05 = 0.45.
    assert!((spec.left_t - 0.35).abs() < 1e-12);
    assert!((spec.right_t - 0.45).abs() < 1e-12);
    // Apex lies inward: edge runs from (0,0) to (0.3, 0), so tangent is (1, 0), inward is (0, 1).
    // Center is (0.12, 0). Apex is (0.12, 0.08).
    assert!((spec.apex - DVec2::new(0.12, 0.08)).length() < 1e-12);

    // Validation of dart parameters
    let validated = poly
        .validate_dart(&spec.edge, spec.left_t, spec.right_t, spec.apex)
        .unwrap();
    assert!((validated.intake_m - 0.03).abs() < 1e-12);
    assert!((validated.depth_m - 0.08).abs() < 1e-12);
    assert!((validated.removed_area_m2 - 0.5 * 0.03 * 0.08).abs() < 1e-12);
    assert!((validated.left - DVec2::new(0.105, 0.0)).length() < 1e-12);
    assert!((validated.right - DVec2::new(0.135, 0.0)).length() < 1e-12);
    // Leg lengths equal by perpendicular bisector
    assert!(
        (validated.left.distance(validated.apex) - validated.right.distance(validated.apex)).abs()
            < 1e-12
    );
}

#[test]
fn dart_geometry_validation_contracts() {
    let poly = plain_rectangle(0.3, 0.2);
    let edge = "bottom";

    // 1. Inverted or equal stations
    assert!(
        poly.validate_dart(edge, 0.45, 0.35, DVec2::new(0.12, 0.08))
            .is_err()
    );
    assert!(
        poly.validate_dart(edge, 0.35, 0.35, DVec2::new(0.105, 0.08))
            .is_err()
    );

    // 2. Out of clearance (station too close to boundary ends)
    assert!(
        poly.validate_dart(edge, 0.005, 0.10, DVec2::new(0.015, 0.08))
            .is_err()
    );
    assert!(
        poly.validate_dart(edge, 0.90, 0.995, DVec2::new(0.28, 0.08))
            .is_err()
    );

    // 3. Station delta too small (< 0.005)
    assert!(
        poly.validate_dart(edge, 0.500, 0.503, DVec2::new(0.15, 0.08))
            .is_err()
    );

    // 4. Intake too small (< 3 mm) or too large (> 150 mm)
    // 0.3 * 0.006 = 1.8 mm (< 3 mm)
    assert!(
        poly.validate_dart(edge, 0.497, 0.503, DVec2::new(0.15, 0.08))
            .is_err()
    );
    // 0.3 * 0.6 = 180 mm (> 150 mm)
    assert!(
        poly.validate_dart(edge, 0.2, 0.8, DVec2::new(0.15, 0.08))
            .is_err()
    );

    // 5. Depth too small (< 8 mm) or too large (> 500 mm)
    assert!(
        poly.validate_dart(edge, 0.35, 0.45, DVec2::new(0.12, 0.005))
            .is_err()
    );
    assert!(
        poly.validate_dart(edge, 0.35, 0.45, DVec2::new(0.12, 0.60))
            .is_err()
    );

    // 6. Apex not on perpendicular bisector
    assert!(
        poly.validate_dart(edge, 0.35, 0.45, DVec2::new(0.15, 0.08))
            .is_err()
    );

    // 7. Non-finite coordinates
    assert!(
        poly.validate_dart(edge, f64::NAN, 0.45, DVec2::new(0.12, 0.08))
            .is_err()
    );
    assert!(
        poly.validate_dart(edge, 0.35, 0.45, DVec2::new(f64::NAN, 0.08))
            .is_err()
    );

    // 8. Unknown edge
    assert!(
        poly.validate_dart("unknown_edge", 0.35, 0.45, DVec2::new(0.12, 0.08))
            .is_err()
    );

    // 9. Edge too short (< 20 mm)
    let tiny_poly = plain_rectangle(0.015, 0.015);
    assert!(
        tiny_poly
            .validate_dart("bottom", 0.35, 0.45, DVec2::new(0.006, 0.008))
            .is_err()
    );
}

#[test]
fn real_dart_authoring_transaction_and_solver_seam() {
    let source = rectangle_template(0.3, 0.2);
    let panel = source.panels[0].id.clone();
    let edge = source.panels[0].polygon.as_ref().unwrap().edges[0].clone();
    let original_bytes = serde_json::to_vec(&source).unwrap();
    let source_area = 0.3 * 0.2;

    // Apply DartCutSew command: intake from 35% to 45% (30 mm), apex at depth 80 mm.
    let cmd = Command::DartCutSew {
        panel: panel.clone(),
        edge: edge.clone(),
        left_t: 0.35,
        right_t: 0.45,
        apex: DVec2::new(0.12, 0.08),
    };
    let edit = authoring::apply(&source, cmd).unwrap();

    // 1. Boundary topology check: 4 points -> 7 points, 4 edges -> 7 edges
    let polygon = edit.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(polygon.points.len(), 7);
    assert_eq!(polygon.edges.len(), 7);

    // 2. Solver seam check: exactly 1 seam coupling leg_a and leg_b with reverse_b = true
    assert_eq!(edit.template.seams.len(), 1);
    let seam = &edit.template.seams[0];
    assert!(seam.id.starts_with("dart_"));
    assert_eq!(seam.a.panel, panel);
    assert_eq!(seam.b.panel, panel);
    assert!(seam.reverse_b);
    assert_eq!(seam.a.range, [0., 1.]);
    assert_eq!(seam.b.range, [0., 1.]);

    // 3. Area reduction: exact geometric triangle (0.5 * 0.03 * 0.08 = 0.0012 m^2)
    let mut double_area = 0.0;
    for i in 0..polygon.points.len() {
        double_area += polygon.points[i].perp_dot(polygon.points[(i + 1) % polygon.points.len()]);
    }
    let rest_area = double_area * 0.5;
    assert!((source_area - rest_area - 0.0012).abs() < 1e-9);

    // 4. Physical simulation compilation & remeshing
    assert_eq!(edit.garment.seams.len(), 1);
    assert!(!edit.garment.seams[0].samples.is_empty());
    let max_gap = edit.garment.seams[0].max_gap(&edit.garment.initial_state.x);
    let rms_gap = (edit.garment.seams[0].integrated_gap_squared(&edit.garment.initial_state.x)
        / edit.garment.seams[0].measure())
    .sqrt();
    assert!(
        max_gap > 0.01,
        "dart intake is visibly open in 3D rest pose"
    );
    assert!(rms_gap > 0. && rms_gap <= max_gap + 1e-10);

    // 5. Serialization round trip
    let json_bytes = serde_json::to_vec(&edit.template).unwrap();
    let reloaded: GarmentTemplate = serde_json::from_slice(&json_bytes).unwrap();
    assert_eq!(reloaded.seams.len(), 1);
    assert!(reloaded.compile().is_ok());

    // 6. History undo / redo
    let mut history = History::default();
    history.record(source.clone());
    let undone = history.restore(&edit.template, false).unwrap();
    assert_eq!(
        serde_json::to_vec(&undone.template).unwrap(),
        original_bytes
    );
    let redone = history.restore(&undone.template, true).unwrap();
    assert_eq!(serde_json::to_vec(&redone.template).unwrap(), json_bytes);
}

#[test]
fn dart_inherits_seam_allowances_across_all_cut_edges() {
    let source = rectangle_template(0.3, 0.2);
    let panel = source.panels[0].id.clone();
    let edge = source.panels[0].polygon.as_ref().unwrap().edges[0].clone();

    // Set 10 mm seam allowance across all boundaries
    let annotated = authoring::apply(
        &source,
        Command::PanelSeamAllowance {
            panel: panel.clone(),
            width_m: 0.01,
        },
    )
    .unwrap()
    .template;

    // Author cut-and-sewn dart
    let edit = authoring::apply(
        &annotated,
        Command::DartCutSew {
            panel: panel.clone(),
            edge: edge.clone(),
            left_t: 0.35,
            right_t: 0.45,
            apex: DVec2::new(0.12, 0.08),
        },
    )
    .unwrap();

    let dart_poly = edit.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(dart_poly.points.len(), 7);
    assert_eq!(dart_poly.edges.len(), 7);
    // Every single edge (including leg_a, leg_b, trailing) inherits the 10 mm allowance!
    assert_eq!(dart_poly.seam_allowances.len(), 7);
    for edge_name in &dart_poly.edges {
        assert_eq!(
            dart_poly.seam_allowances.get(edge_name).copied(),
            Some(0.01),
            "Edge {edge_name} must inherit allowance"
        );
    }

    // The cutting contour must build cleanly with the dart
    let cutting = dart_poly.cutting_contour().unwrap().unwrap();
    assert!(!cutting.outer.is_empty());
}

#[test]
fn dart_placement_refusals_are_atomic_and_safe() {
    // 1. Refuse dart on already sewn boundary
    let tshirt = presets::basic(GarmentKind::Tshirt, 0);
    let before = serde_json::to_vec(&tshirt).unwrap();
    assert!(
        tshirt
            .seams
            .iter()
            .any(|s| s.a.edge.as_deref() == Some("side-right"))
    );
    let res = authoring::apply(
        &tshirt,
        Command::DartCutSew {
            panel: "front".into(),
            edge: "side-right".into(),
            left_t: 0.3,
            right_t: 0.4,
            apex: DVec2::new(0.1, 0.2),
        },
    );
    assert!(res.is_err(), "Cannot cut dart on sewn boundary");
    assert_eq!(serde_json::to_vec(&tshirt).unwrap(), before);

    // 2. Refuse dart on boundary with notches
    let mut notched = rectangle_template(0.3, 0.2);
    let panel = notched.panels[0].id.clone();
    let edge = notched.panels[0].polygon.as_ref().unwrap().edges[0].clone();
    notched = authoring::apply(
        &notched,
        Command::SetNotch {
            panel: panel.clone(),
            edge: edge.clone(),
            index: None,
            notch: Notch::new(0.5, 0.005, NotchKind::V),
        },
    )
    .unwrap()
    .template;
    let before_notched = serde_json::to_vec(&notched).unwrap();
    let res_notched = authoring::apply(
        &notched,
        Command::DartCutSew {
            panel: panel.clone(),
            edge: edge.clone(),
            left_t: 0.35,
            right_t: 0.45,
            apex: DVec2::new(0.12, 0.08),
        },
    );
    assert!(
        res_notched.is_err(),
        "Dart on boundary with notches must be refused"
    );
    assert_eq!(serde_json::to_vec(&notched).unwrap(), before_notched);
}

// =========================================================================
// SECTION 2: SEAM ALLOWANCE GUIDES AND PREVIEW
// =========================================================================

#[test]
fn panel_seam_allowance_transaction_and_clearing() {
    let source = rectangle_template(0.4, 0.3);
    let panel = source.panels[0].id.clone();

    // 1. Set 15 mm allowance across all outer edges of the panel
    let edit = authoring::apply(
        &source,
        Command::PanelSeamAllowance {
            panel: panel.clone(),
            width_m: 0.015,
        },
    )
    .unwrap();
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(poly.seam_allowances.len(), 4);
    for name in &poly.edges {
        assert_eq!(poly.seam_allowances[name], 0.015);
    }

    // 2. Clear all allowances with width_m = 0.0
    let cleared = authoring::apply(
        &edit.template,
        Command::PanelSeamAllowance {
            panel: panel.clone(),
            width_m: 0.0,
        },
    )
    .unwrap();
    let poly_cleared = cleared.template.panels[0].polygon.as_ref().unwrap();
    assert!(poly_cleared.seam_allowances.is_empty());

    // 3. Undo restores all 4 allowances
    let mut history = History::default();
    history.record(edit.template.clone());
    let restored = history.restore(&cleared.template, false).unwrap();
    let poly_restored = restored.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(poly_restored.seam_allowances.len(), 4);
    assert_eq!(poly_restored.seam_allowances[&poly.edges[0]], 0.015);
}

#[test]
fn allowance_guide_single_boundary_computation() {
    let mut poly = plain_rectangle(0.5, 0.4);
    poly.seam_allowances.insert("bottom".into(), 0.012);

    // Single edge cutting guide
    let guide = poly.allowance_guide("bottom").unwrap().unwrap();
    assert_eq!(guide.len(), 2);
    // "bottom" goes from (0,0) to (0.5,0). Outward normal points to the right: (0, -1).
    assert!((guide[0] - DVec2::new(0.0, -0.012)).length() < 1e-12);
    assert!((guide[1] - DVec2::new(0.5, -0.012)).length() < 1e-12);

    // Edge without allowance returns None
    assert!(poly.allowance_guide("top").unwrap().is_none());

    // Invalid edge returns error
    assert!(poly.allowance_guide("nonexistent").is_err());
}

#[test]
fn cutting_contour_miter_joins_and_area_expansion() {
    let mut poly = plain_rectangle(1.0, 1.0);
    // 10 mm on all 4 edges
    poly.set_panel_seam_allowance(0.01).unwrap();

    let contour = poly.cutting_contour().unwrap().unwrap();
    assert_eq!(contour.outer.len(), 4);
    // Corners expand outward by sqrt(2) * 10 mm to form a 1.02 x 1.02 m square
    let expected_area = 1.02 * 1.02;
    let actual_area: f64 = contour
        .outer
        .iter()
        .enumerate()
        .map(|(i, p)| p.perp_dot(contour.outer[(i + 1) % 4]))
        .sum::<f64>()
        * 0.5;
    assert!((actual_area - expected_area).abs() < 1e-9);

    // Verify outer corner coordinates: (-0.01, -0.01), (1.01, -0.01), (1.01, 1.01), (-0.01, 1.01)
    assert!(
        contour
            .outer
            .iter()
            .any(|p| (p - DVec2::new(-0.01, -0.01)).length() < 1e-9)
    );
    assert!(
        contour
            .outer
            .iter()
            .any(|p| (p - DVec2::new(1.01, -0.01)).length() < 1e-9)
    );
    assert!(
        contour
            .outer
            .iter()
            .any(|p| (p - DVec2::new(1.01, 1.01)).length() < 1e-9)
    );
    assert!(
        contour
            .outer
            .iter()
            .any(|p| (p - DVec2::new(-0.01, 1.01)).length() < 1e-9)
    );
}

#[test]
fn cutting_contour_unequal_allowances_miter_correctly() {
    let mut poly = plain_rectangle(1.0, 1.0);
    // bottom = 20 mm, right = 10 mm, top = 20 mm, left = 10 mm
    poly.seam_allowances.insert("bottom".into(), 0.02);
    poly.seam_allowances.insert("right".into(), 0.01);
    poly.seam_allowances.insert("top".into(), 0.02);
    poly.seam_allowances.insert("left".into(), 0.01);

    let contour = poly.cutting_contour().unwrap().unwrap();
    assert_eq!(contour.outer.len(), 4);
    // Bottom-right corner: x moves right by 10 mm (1.01), y moves down by 20 mm (-0.02)
    assert!(
        contour
            .outer
            .iter()
            .any(|p| (p - DVec2::new(1.01, -0.02)).length() < 1e-9)
    );
    // Top-left corner: x moves left by 10 mm (-0.01), y moves up by 20 mm (1.02)
    assert!(
        contour
            .outer
            .iter()
            .any(|p| (p - DVec2::new(-0.01, 1.02)).length() < 1e-9)
    );
    // Area = 1.02 * 1.04 = 1.0608 m^2
    let actual_area: f64 = contour
        .outer
        .iter()
        .enumerate()
        .map(|(i, p)| p.perp_dot(contour.outer[(i + 1) % 4]))
        .sum::<f64>()
        * 0.5;
    assert!((actual_area - 1.0608).abs() < 1e-9);
}

#[test]
fn cutting_contour_acute_corner_miter_limit_bevel() {
    // Sharp triangle with a very acute apex (angle < 15 degrees)
    let poly = PolygonPattern {
        points: vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.2, 0.0),
            DVec2::new(0.1, 1.0), // sharp point at top
        ],
        edges: vec!["base".into(), "right".into(), "left".into()],
        subdivisions: 0,
        meshing: None,
        curves: Default::default(),
        holes: vec![],
        lines: vec![],
        target_lengths: Default::default(),
        seam_allowances: [
            ("base".into(), 0.01),
            ("right".into(), 0.01),
            ("left".into(), 0.01),
        ]
        .into(),
        notches: Default::default(),
        measurements: vec![],
    };

    // The apex miter would spike past MAX_MITER_RATIO (8x = 80 mm); it must fall back to a 2-point bevel.
    let contour = poly.cutting_contour().unwrap().unwrap();
    // 3 base corners + 1 bevel point at apex = 4 points total
    assert_eq!(
        contour.outer.len(),
        4,
        "Acute corner must bevel into 2 vertices"
    );
}

#[test]
fn cutting_contour_preview_renders_in_svg() {
    let source = rectangle_template(0.3, 0.2);
    let panel = source.panels[0].id.clone();
    let marked = authoring::apply(
        &source,
        Command::PanelSeamAllowance {
            panel: panel.clone(),
            width_m: 0.012,
        },
    )
    .unwrap()
    .template;

    let svg = export::pattern_svg(&marked);
    assert!(svg.contains("data-role=\"cutting-line\""));
    assert!(svg.contains("Cutting line"));
}

// =========================================================================
// SECTION 3: SEWING NOTCHES
// =========================================================================

#[test]
fn slit_notch_marker_material_coordinates() {
    let poly = plain_rectangle(0.6, 0.4);
    // Slit notch at midpoint (at = 0.5) with depth 6 mm (0.006 m) on bottom edge
    let notch = Notch::new(0.5, 0.006, NotchKind::Slit);
    let marker = poly.notch_marker("bottom", &notch).unwrap();
    assert_eq!(marker.len(), 2);
    // Origin is at (0.3, 0.0); outward normal is (0, -1); tip is at (0.3, -0.006)
    assert!((marker[0] - DVec2::new(0.3, 0.0)).length() < 1e-12);
    assert!((marker[1] - DVec2::new(0.3, -0.006)).length() < 1e-12);

    // notch_segment returns centerline segment
    let seg = poly.notch_segment("bottom", &notch).unwrap();
    assert_eq!(seg, [marker[0], marker[1]]);
}

#[test]
fn v_wedge_notch_bases_follow_boundary_arclength() {
    let poly = plain_rectangle(0.6, 0.4);
    // V notch at center (at = 0.5) with length 10 mm (0.01 m)
    let notch = Notch::new(0.5, 0.01, NotchKind::V);
    let marker = poly.notch_marker("bottom", &notch).unwrap();
    // 4 points for closed triangle polyline [left_base, tip, right_base, left_base]
    assert_eq!(marker.len(), 4);
    // Center is (0.3, 0.0). Half length = 5 mm = 0.005 m.
    // Left base = (0.295, 0.0), Right base = (0.305, 0.0), Tip = (0.300, -0.010).
    assert!((marker[0] - DVec2::new(0.295, 0.0)).length() < 1e-12);
    assert!((marker[1] - DVec2::new(0.300, -0.010)).length() < 1e-12);
    assert!((marker[2] - DVec2::new(0.305, 0.0)).length() < 1e-12);
    assert_eq!(marker[3], marker[0]);
}

#[test]
fn notch_clearance_end_rule_and_corner_crossing_rejection() {
    let poly = plain_rectangle(0.6, 0.4);

    // 1. Slit notch within 1 mm of boundary end rejected
    let bad_slit_start = Notch::new(0.0005 / 0.6, 0.005, NotchKind::Slit);
    assert!(poly.notch_marker("bottom", &bad_slit_start).is_err());
    let bad_slit_end = Notch::new(1.0 - 0.0005 / 0.6, 0.005, NotchKind::Slit);
    assert!(poly.notch_marker("bottom", &bad_slit_end).is_err());

    // 2. Slit notch exactly 1 mm inside accepted
    let valid_slit = Notch::new(0.001 / 0.6, 0.005, NotchKind::Slit);
    assert!(poly.notch_marker("bottom", &valid_slit).is_ok());

    // 3. V notch center at 4 mm from end, length 10 mm:
    // Left base would be at 4 - 5 = -1 mm (crossing corner!). Refused!
    let bad_v = Notch::new(0.004 / 0.6, 0.010, NotchKind::V);
    assert!(poly.notch_marker("bottom", &bad_v).is_err());

    // 4. V notch center at 6 mm from end, length 10 mm:
    // Left base is at 6 - 5 = 1 mm (exactly 1 mm clearance). Accepted!
    let valid_v = Notch::new(0.006 / 0.6, 0.010, NotchKind::V);
    assert!(poly.notch_marker("bottom", &valid_v).is_ok());
}

#[test]
fn paired_notches_atomic_transaction() {
    let source = rectangle_template(0.4, 0.3);
    let panel = source.panels[0].id.clone();
    let edge = source.panels[0].polygon.as_ref().unwrap().edges[0].clone();
    let original = serde_json::to_vec(&source).unwrap();

    // Add paired notches: center at t = 0.5, spacing = 8 mm (0.008 m), depth = 5 mm
    let edit = authoring::apply(
        &source,
        Command::NotchPair {
            panel: panel.clone(),
            edge: edge.clone(),
            center_t: 0.5,
            spacing_m: 0.008,
            depth_m: 0.005,
            kind: NotchKind::V,
        },
    )
    .unwrap();

    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    let marks = poly.notches.get(&edge).unwrap();
    assert_eq!(marks.len(), 2, "Atomic pair inserts two notches");

    // Edge length = 0.4 m. Half spacing = 4 mm = 0.004 m = 0.01 normalized t.
    // t_a = 0.5 - 0.01 = 0.49; t_b = 0.5 + 0.01 = 0.51.
    assert!((marks[0].at - 0.49).abs() < 1e-12);
    assert!((marks[1].at - 0.51).abs() < 1e-12);
    assert_eq!(marks[0].length_m, 0.005);
    assert_eq!(marks[1].length_m, 0.005);
    assert_eq!(marks[0].kind, NotchKind::V);
    assert_eq!(marks[1].kind, NotchKind::V);

    // Distance between them in metres = (0.51 - 0.49) * 0.4 = 0.008 m
    let delta_m = (marks[1].at - marks[0].at) * 0.4;
    assert!((delta_m - 0.008).abs() < 1e-12);

    // Single history step undoes both notches
    let mut history = History::default();
    history.record(source.clone());
    let undone = history.restore(&edit.template, false).unwrap();
    assert_eq!(serde_json::to_vec(&undone.template).unwrap(), original);
    assert!(!history.can_undo(), "One undo must remove both notches");

    // Redo restores both
    let redone = history.restore(&undone.template, true).unwrap();
    assert_eq!(
        serde_json::to_vec(&redone.template).unwrap(),
        serde_json::to_vec(&edit.template).unwrap()
    );
}

#[test]
fn paired_notches_validation_refusals() {
    let source = rectangle_template(0.4, 0.3);
    let panel = source.panels[0].id.clone();
    let edge = source.panels[0].polygon.as_ref().unwrap().edges[0].clone();
    let before = serde_json::to_vec(&source).unwrap();

    // Spacing too large for boundary (> 200 mm or exceeds edge clearance)
    for (center, spacing, depth) in [
        (0.5, 0.39, 0.005),
        (0.5, 0.0, 0.005),
        (0.002, 0.008, 0.005),
        (0.998, 0.008, 0.005),
        (f64::NAN, 0.008, 0.005),
    ] {
        let res = authoring::apply(
            &source,
            Command::NotchPair {
                panel: panel.clone(),
                edge: edge.clone(),
                center_t: center,
                spacing_m: spacing,
                depth_m: depth,
                kind: NotchKind::Slit,
            },
        );
        assert!(
            res.is_err(),
            "Invalid notch pair {center} {spacing} {depth}"
        );
        assert_eq!(serde_json::to_vec(&source).unwrap(), before);
    }
}

#[test]
fn export_preview_renders_notches_in_svg() {
    let source = rectangle_template(0.4, 0.3);
    let panel = source.panels[0].id.clone();
    let edge = source.panels[0].polygon.as_ref().unwrap().edges[0].clone();
    let marked = authoring::apply(
        &source,
        Command::SetNotch {
            panel: panel.clone(),
            edge: edge.clone(),
            index: None,
            notch: Notch::new(0.5, 0.008, NotchKind::V),
        },
    )
    .unwrap()
    .template;

    let svg = export::pattern_svg(&marked);
    assert!(svg.contains("data-role=\"notch\""));
    assert!(svg.contains("notch 1"));
}

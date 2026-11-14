use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command, History},
    garment::{
        GarmentTemplate, curves,
        meshing::MeshingSettings,
        shapes::{self, MirrorAxis, Primitive},
    },
};

fn fixture() -> GarmentTemplate {
    authoring::apply(
        &GarmentTemplate::two_panel_demo(),
        Command::Primitive {
            kind: Primitive::Rectangle,
            size: DVec2::new(0.4, 0.5),
            sweep_degrees: 180.0,
            origin: DVec2::ZERO,
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template
}
fn command(t: &GarmentTemplate, panel: usize, radius_m: f64) -> Command {
    Command::RoundCorner {
        panel: t.panels[panel].id.clone(),
        edge: t.panels[panel].polygon.as_ref().unwrap().edges[1].clone(),
        radius_m,
    }
}
#[test]
fn ten_mm_fillet_has_expected_tangencies_and_circle_error() {
    let t = fixture();
    let old = t.panels[0].polygon.as_ref().unwrap();
    let names = old.edges.clone();
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    assert!(edit.reset_simulation);
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(poly.edges[0], names[0]);
    assert_eq!(poly.edges[2], names[1]);
    assert!(poly.points[1].distance(DVec2::new(0.39, 0.0)) < 1e-12);
    assert!(poly.points[2].distance(DVec2::new(0.4, 0.01)) < 1e-12);
    let controls = poly.curves[&poly.edges[1]];
    let cubic = [poly.points[1], controls[0], controls[1], poly.points[2]];
    let center = DVec2::new(0.39, 0.01);
    let max_error = (0..=10000)
        .map(|i| (curves::cubic(cubic, i as f64 / 10000.0).distance(center) - 0.01).abs())
        .fold(0.0_f64, f64::max);
    assert!(max_error < 0.000_002_73, "error in metres: {max_error}");
    let mut history = History::default();
    history.record(t.clone());
    let undo = history.restore(&edit.template, false).unwrap();
    assert_eq!(
        serde_json::to_vec(&undo.template).unwrap(),
        serde_json::to_vec(&t).unwrap()
    );
    let redo = history.restore(&undo.template, true).unwrap();
    assert_eq!(
        serde_json::to_vec(&redo.template).unwrap(),
        serde_json::to_vec(&edit.template).unwrap()
    );
    let json = serde_json::to_vec(&redo.template).unwrap();
    let loaded: GarmentTemplate = serde_json::from_slice(&json).unwrap();
    loaded.compile().unwrap();
}
#[test]
fn rejects_invalid_and_referenced_inputs_without_mutation() {
    let t = fixture();
    let before = serde_json::to_vec(&t).unwrap();
    for r in [0.0, -0.01, f64::NAN, f64::INFINITY, 1.0] {
        assert!(authoring::apply(&t, command(&t, 0, r)).is_err());
        assert_eq!(before, serde_json::to_vec(&t).unwrap());
    }
    let mut referenced = t.clone();
    let poly = referenced.panels[0].polygon.as_mut().unwrap();
    poly.target_lengths.insert(poly.edges[1].clone(), 0.2);
    assert!(authoring::apply(&referenced, command(&referenced, 0, 0.01)).is_err());
    let rounded = authoring::apply(&t, command(&t, 0, 0.01)).unwrap().template;
    let poly = rounded.panels[0].polygon.as_ref().unwrap();
    assert!(
        authoring::apply(
            &rounded,
            Command::RoundCorner {
                panel: rounded.panels[0].id.clone(),
                edge: poly.edges[1].clone(),
                radius_m: 0.01
            }
        )
        .is_err()
    );
}
#[test]
fn linked_rounding_works_from_either_partner_and_refuses_partner_metadata() {
    for index in 0..2 {
        let base = fixture();
        let t = authoring::apply(
            &base,
            Command::LinkedMirror {
                panel: base.panels[0].id.clone(),
                axis: MirrorAxis::X,
            },
        )
        .unwrap()
        .template;
        let edit = authoring::apply(&t, command(&t, index, 0.01)).unwrap();
        let a = &edit.template.panels[index];
        let b = &edit.template.panels[1 - index];
        let mut expected = a.polygon.clone().unwrap();
        shapes::mirror(
            &mut expected,
            MirrorAxis::X,
            DVec2::new(a.pattern.width, a.pattern.height),
        );
        assert_eq!(
            serde_json::to_value(expected).unwrap(),
            serde_json::to_value(&b.polygon).unwrap()
        );
        let mut blocked = t.clone();
        let p = blocked.panels[1 - index].polygon.as_mut().unwrap();
        p.target_lengths.insert(p.edges[0].clone(), 0.1);
        let before = serde_json::to_vec(&blocked).unwrap();
        assert!(authoring::apply(&blocked, command(&blocked, index, 0.01)).is_err());
        assert_eq!(before, serde_json::to_vec(&blocked).unwrap());
    }
}

#[test]
fn linked_rounding_that_changes_material_frame_is_atomic() {
    let base = authoring::apply(
        &fixture(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.1),
                DVec2::new(0.3, 0.5),
                DVec2::new(0.0, 0.4),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let t = authoring::apply(
        &base,
        Command::LinkedMirror {
            panel: base.panels[0].id.clone(),
            axis: MirrorAxis::X,
        },
    )
    .unwrap()
    .template;
    let before = serde_json::to_vec(&t).unwrap();
    let error = authoring::apply(&t, command(&t, 0, 0.01)).err().unwrap();
    assert!(error.contains("material frame"), "{error}");
    assert_eq!(before, serde_json::to_vec(&t).unwrap());
}

#[test]
fn rounding_preserves_automatic_board_origins() {
    let mut t = fixture();
    for _ in 0..2 {
        t = authoring::apply(
            &t,
            Command::Duplicate {
                panel: t.panels[0].id.clone(),
            },
        )
        .unwrap()
        .template;
    }
    for panel in &mut t.panels {
        panel.layout_2d = None;
    }
    let origins: Vec<_> = (0..t.panels.len())
        .map(|i| authoring::layout(&t, i))
        .collect();
    let edit = authoring::apply(&t, command(&t, 1, 0.01)).unwrap();
    for (i, origin) in origins.iter().enumerate() {
        assert_eq!(authoring::layout(&edit.template, i), *origin);
    }
    // A corner that changes the first piece's width must not move later pieces.
    let skewed = authoring::apply(
        &fixture(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.1),
                DVec2::new(0.3, 0.5),
                DVec2::new(0.0, 0.4),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let mut t = authoring::apply(
        &skewed,
        Command::Duplicate {
            panel: skewed.panels[0].id.clone(),
        },
    )
    .unwrap()
    .template;
    for panel in &mut t.panels {
        panel.layout_2d = None;
    }
    let next_origin = authoring::layout(&t, 1);
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    assert!(edit.template.panels[0].pattern.width < t.panels[0].pattern.width);
    assert_eq!(authoring::layout(&edit.template, 1), next_origin);
}

#[test]
fn generated_boundary_name_is_bounded_and_avoids_collisions() {
    let mut t = fixture();
    let poly = t.panels[0].polygon.as_mut().unwrap();
    poly.edges[0] = "corner_round_1".into();
    poly.edges[1] = "e".repeat(80);
    t.compile().unwrap();
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    assert_eq!(poly.edges[1], "corner_round_2");
    assert_eq!(poly.edges[2], "e".repeat(80));
    edit.template.compile().unwrap();
}

#[test]
fn forty_five_degree_fillet_has_correct_tangencies_and_radius() {
    let t = authoring::apply(
        &fixture(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.0),
                DVec2::new(0.6, 0.2),
                DVec2::new(0.6, 0.5),
                DVec2::new(0.0, 0.5),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;
    let edit = authoring::apply(&t, command(&t, 0, 0.01)).unwrap();
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    let trim = 0.01 * (std::f64::consts::PI / 8.0).tan();
    let corner = DVec2::new(0.4, 0.0);
    let direction = DVec2::new(1.0, 1.0).normalize();
    assert!(poly.points[1].distance(corner - DVec2::X * trim) < 1e-12);
    assert!(poly.points[2].distance(corner + direction * trim) < 1e-12);
    let controls = poly.curves[&poly.edges[1]];
    let cubic = [poly.points[1], controls[0], controls[1], poly.points[2]];
    let center = poly.points[1] + DVec2::Y * 0.01;
    let error = (0..=10000)
        .map(|i| (curves::cubic(cubic, i as f64 / 10000.0).distance(center) - 0.01).abs())
        .fold(0.0_f64, f64::max);
    assert!(error < 0.000_002_73, "error in metres: {error}");
}

#[test]
fn shape_mode_varies_curvature_and_preserves_error_at_nominal_tension() {
    let t = fixture();
    let panel = t.panels[0].id.clone();
    let edge = t.panels[0].polygon.as_ref().unwrap().edges[1].clone();

    // Nominal tension (0.55228...) produces the exact circle approximation
    let edit_nominal = authoring::apply(
        &t,
        Command::RoundCornerShape {
            panel: panel.clone(),
            edge: edge.clone(),
            radius_m: 0.01,
            tension: authoring::corner_tool::CIRCLE_TENSION,
            rule: authoring::CornerRule::Smooth,
        },
    )
    .unwrap();
    let poly_nom = edit_nominal.template.panels[0].polygon.as_ref().unwrap();
    let ctrl_nom = poly_nom.curves[&poly_nom.edges[1]];
    let cubic_nom = [
        poly_nom.points[1],
        ctrl_nom[0],
        ctrl_nom[1],
        poly_nom.points[2],
    ];
    let center = DVec2::new(0.39, 0.01);
    let err_nom = (0..=10000)
        .map(|i| (curves::cubic(cubic_nom, i as f64 / 10000.0).distance(center) - 0.01).abs())
        .fold(0.0_f64, f64::max);
    assert!(err_nom < 0.000_002_73);

    // Flatter curve with lower tension (0.25)
    let edit_flat = authoring::apply(
        &t,
        Command::RoundCornerShape {
            panel: panel.clone(),
            edge: edge.clone(),
            radius_m: 0.01,
            tension: 0.25,
            rule: authoring::CornerRule::Smooth,
        },
    )
    .unwrap();
    let poly_flat = edit_flat.template.panels[0].polygon.as_ref().unwrap();
    let ctrl_flat = poly_flat.curves[&poly_flat.edges[1]];
    let handle_nom_len = (ctrl_nom[0] - poly_nom.points[1]).length();
    let handle_flat_len = (ctrl_flat[0] - poly_flat.points[1]).length();
    assert!(handle_flat_len < handle_nom_len);

    // Squarer curve with higher tension (0.85)
    let edit_square = authoring::apply(
        &t,
        Command::RoundCornerShape {
            panel: panel.clone(),
            edge: edge.clone(),
            radius_m: 0.01,
            tension: 0.85,
            rule: authoring::CornerRule::Smooth,
        },
    )
    .unwrap();
    let poly_square = edit_square.template.panels[0].polygon.as_ref().unwrap();
    let ctrl_square = poly_square.curves[&poly_square.edges[1]];
    let handle_square_len = (ctrl_square[0] - poly_square.points[1]).length();
    assert!(handle_square_len > handle_nom_len);
}

#[test]
fn handle_adjustments_enforce_sharp_smooth_and_symmetric_rules() {
    let t = fixture();
    let panel = t.panels[0].id.clone();
    let edge = t.panels[0].polygon.as_ref().unwrap().edges[2].clone(); // top-right corner

    let rounded = authoring::apply(
        &t,
        Command::RoundCorner {
            panel: panel.clone(),
            edge: edge.clone(),
            radius_m: 0.01,
        },
    )
    .unwrap()
    .template;

    let poly = rounded.panels[0].polygon.as_ref().unwrap();
    let fillet_edge = poly.edges[2].clone();
    let anchor = poly.points[2];
    let original_ctrl0 = poly.curves[&fillet_edge][0];

    // Sharp: incident handles move independently
    let sharp_target = original_ctrl0 + DVec2::new(-0.002, 0.002);
    let edit_sharp = authoring::apply(
        &rounded,
        Command::AdjustCornerHandle {
            panel: panel.clone(),
            edge: fillet_edge.clone(),
            handle_index: 0,
            position: sharp_target,
            rule: authoring::CornerRule::Sharp,
        },
    )
    .unwrap();
    let poly_sharp = edit_sharp.template.panels[0].polygon.as_ref().unwrap();
    assert!((poly_sharp.curves[&fillet_edge][0] - sharp_target).length() < 1e-12);
    // Edge 1 was straight; in Sharp mode it remains straight
    assert!(!poly_sharp.curves.contains_key(&poly_sharp.edges[1]));

    // Symmetric: opposite collinear handles with equal lengths
    let symmetric_target = anchor + DVec2::new(-0.004, 0.003); // length 0.005
    let edit_sym = authoring::apply(
        &rounded,
        Command::AdjustCornerHandle {
            panel: panel.clone(),
            edge: fillet_edge.clone(),
            handle_index: 0,
            position: symmetric_target,
            rule: authoring::CornerRule::Symmetric,
        },
    )
    .unwrap();
    let poly_sym = edit_sym.template.panels[0].polygon.as_ref().unwrap();
    assert!((poly_sym.curves[&fillet_edge][0] - symmetric_target).length() < 1e-12);
    // Edge 1 is now curved with handle pointing opposite to symmetric_target with equal length (0.005)
    let opp_handle = poly_sym.curves[&poly_sym.edges[1]][1];
    let expected_opp = anchor - DVec2::new(-0.004, 0.003);
    assert!((opp_handle - expected_opp).length() < 1e-12);
    assert!(((opp_handle - anchor).length() - 0.005).abs() < 1e-12);

    // Smooth: opposite collinear handles with independent lengths
    let smooth_target = anchor + DVec2::new(-0.008, 0.006); // length 0.010
    let edit_smooth = authoring::apply(
        &edit_sym.template,
        Command::AdjustCornerHandle {
            panel: panel.clone(),
            edge: fillet_edge.clone(),
            handle_index: 0,
            position: smooth_target,
            rule: authoring::CornerRule::Smooth,
        },
    )
    .unwrap();
    let poly_smooth = edit_smooth.template.panels[0].polygon.as_ref().unwrap();
    let new_opp_handle = poly_smooth.curves[&poly_smooth.edges[1]][1];
    let dir = (smooth_target - anchor).normalize();
    let opp_dir = (new_opp_handle - anchor).normalize();
    // Opposite collinear: dir + opp_dir == 0
    assert!((dir + opp_dir).length() < 1e-10);
    // Retains its previous length (0.005)
    assert!(((new_opp_handle - anchor).length() - 0.005).abs() < 1e-10);
}

fn two_panel_sewn_fixture() -> GarmentTemplate {
    use marvelous_cloth::garment::seam::{BoundaryRef, Side};
    let base = fixture();
    let dup = authoring::apply(
        &base,
        Command::Duplicate {
            panel: base.panels[0].id.clone(),
        },
    )
    .unwrap()
    .template;
    let p1 = &dup.panels[0];
    let p2 = &dup.panels[1];
    let poly1 = p1.polygon.as_ref().unwrap();
    let poly2 = p2.polygon.as_ref().unwrap();
    authoring::apply(
        &dup,
        Command::Sew {
            a: BoundaryRef {
                reversed: false,
                panel: p1.id.clone(),
                side: Side::Right,
                edge: Some(poly1.edges[1].clone()),
                range: [0.0, 1.0],
            },
            b: BoundaryRef {
                reversed: false,
                panel: p2.id.clone(),
                side: Side::Left,
                edge: Some(poly2.edges[3].clone()),
                range: [0.0, 1.0],
            },
            reverse: false,
            ease: false,
        },
    )
    .unwrap()
    .template
}

#[test]
fn synchronized_cutout_spans_35_to_45_percent_across_sewn_boundaries() {
    let t = two_panel_sewn_fixture();
    let seam_id = t.seams[0].id.clone();
    let source_panel = t.seams[0].a.panel.clone();
    let target_panel = t.seams[0].b.panel.clone();

    let params = authoring::SynchronizedCutoutParams {
        seam_id: seam_id.clone(),
        source_panel: source_panel.clone(),
        interval: [0.35, 0.45],
        depth_m: 0.015,
        shape: authoring::CutoutShape::VNotch,
        policy: authoring::CutoutPolicy::Correspondence,
        seam_policy: authoring::CutoutSeamPolicy::SewCutout,
    };

    let edit = authoring::apply(&t, Command::SynchronizedCutout(params.clone())).unwrap();
    assert!(edit.reset_simulation);

    let p_src = edit
        .template
        .panels
        .iter()
        .find(|p| p.id == source_panel)
        .unwrap();
    let p_tgt = edit
        .template
        .panels
        .iter()
        .find(|p| p.id == target_panel)
        .unwrap();

    let poly_src = p_src.polygon.as_ref().unwrap();
    let poly_tgt = p_tgt.polygon.as_ref().unwrap();

    // Both panels gained cutout vertices (V-notch adds vertices)
    assert!(poly_src.points.len() > 4);
    assert!(poly_tgt.points.len() > 4);

    // Seam was split into lead, trail, and sewn cutout rims
    assert!(edit.template.seams.iter().any(|s| s.id.contains("lead")));
    assert!(edit.template.seams.iter().any(|s| s.id.contains("trail")));
    assert!(edit.template.seams.iter().any(|s| s.id.contains("rim")));

    // Test Open seam policy leaves rims unsewn
    let mut params_open = params;
    params_open.seam_policy = authoring::CutoutSeamPolicy::Open;
    let edit_open = authoring::apply(&t, Command::SynchronizedCutout(params_open)).unwrap();
    assert!(
        !edit_open
            .template
            .seams
            .iter()
            .any(|s| s.id.contains("rim"))
    );
    assert!(
        edit_open
            .template
            .seams
            .iter()
            .any(|s| s.id.contains("lead"))
    );
    assert!(
        edit_open
            .template
            .seams
            .iter()
            .any(|s| s.id.contains("trail"))
    );

    // Undo / Redo roundtrip
    let mut history = History::default();
    history.record(t.clone());
    let undo = history.restore(&edit.template, false).unwrap();
    assert_eq!(
        serde_json::to_vec(&undo.template).unwrap(),
        serde_json::to_vec(&t).unwrap()
    );
    let redo = history.restore(&undo.template, true).unwrap();
    assert_eq!(
        serde_json::to_vec(&redo.template).unwrap(),
        serde_json::to_vec(&edit.template).unwrap()
    );
    let json = serde_json::to_vec(&redo.template).unwrap();
    let loaded: GarmentTemplate = serde_json::from_slice(&json).unwrap();
    loaded.compile().unwrap();
}

#[test]
fn synchronized_cutout_reversed_seam_alignment() {
    let mut t = two_panel_sewn_fixture();
    t.seams[0].reverse_b = true;

    let seam_id = t.seams[0].id.clone();
    let source_panel = t.seams[0].a.panel.clone();

    let params = authoring::SynchronizedCutoutParams {
        seam_id: seam_id.clone(),
        source_panel: source_panel.clone(),
        interval: [0.35, 0.45],
        depth_m: 0.015,
        shape: authoring::CutoutShape::Rectangular,
        policy: authoring::CutoutPolicy::Correspondence,
        seam_policy: authoring::CutoutSeamPolicy::Open,
    };

    let edit = authoring::apply(&t, Command::SynchronizedCutout(params)).unwrap();
    edit.template.compile().unwrap();
}

#[test]
fn synchronized_cutout_rejects_invalid_inputs_without_mutation() {
    let t = two_panel_sewn_fixture();
    let seam_id = t.seams[0].id.clone();
    let source_panel = t.seams[0].a.panel.clone();
    let before = serde_json::to_vec(&t).unwrap();

    // Invalid interval
    let bad_interval = authoring::SynchronizedCutoutParams {
        seam_id: seam_id.clone(),
        source_panel: source_panel.clone(),
        interval: [0.6, 0.4],
        depth_m: 0.01,
        shape: authoring::CutoutShape::VNotch,
        policy: authoring::CutoutPolicy::Correspondence,
        seam_policy: authoring::CutoutSeamPolicy::Open,
    };
    assert!(authoring::apply(&t, Command::SynchronizedCutout(bad_interval)).is_err());
    assert_eq!(before, serde_json::to_vec(&t).unwrap());

    // Invalid depth
    let bad_depth = authoring::SynchronizedCutoutParams {
        seam_id: seam_id.clone(),
        source_panel: source_panel.clone(),
        interval: [0.3, 0.4],
        depth_m: -0.01,
        shape: authoring::CutoutShape::VNotch,
        policy: authoring::CutoutPolicy::Correspondence,
        seam_policy: authoring::CutoutSeamPolicy::Open,
    };
    assert!(authoring::apply(&t, Command::SynchronizedCutout(bad_depth)).is_err());
    assert_eq!(before, serde_json::to_vec(&t).unwrap());

    // Non-existent seam
    let bad_seam = authoring::SynchronizedCutoutParams {
        seam_id: "missing_seam".into(),
        source_panel: source_panel.clone(),
        interval: [0.3, 0.4],
        depth_m: 0.01,
        shape: authoring::CutoutShape::VNotch,
        policy: authoring::CutoutPolicy::Correspondence,
        seam_policy: authoring::CutoutSeamPolicy::Open,
    };
    assert!(authoring::apply(&t, Command::SynchronizedCutout(bad_seam)).is_err());
    assert_eq!(before, serde_json::to_vec(&t).unwrap());
}

#[test]
fn obtuse_and_concave_corners_round_with_matching_preview() {
    use marvelous_cloth::authoring::{
        CornerRule,
        corner_tool::{CIRCLE_TENSION, preview_fillet, validate_fillet_preview},
    };

    // Concave & obtuse non-orthogonal polygon
    let t = authoring::apply(
        &fixture(),
        Command::Polygon {
            points: vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(0.40, 0.0),
                DVec2::new(0.28, 0.22), // concave inward notch (turn < 0)
                DVec2::new(0.45, 0.50), // obtuse turn (> 90 deg)
                DVec2::new(0.05, 0.45),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.08,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: true,
        },
    )
    .unwrap()
    .template;

    for corner_idx in [2_usize, 3] {
        let panel = &t.panels[0];
        let poly = panel.polygon.as_ref().unwrap();
        let edge_name = poly.edges[corner_idx].clone();
        let radius_m = 0.015;
        let prev =
            validate_fillet_preview(&t, &panel.id, &edge_name, radius_m, CIRCLE_TENSION).unwrap();
        assert!(prev.max_radius_m > radius_m);

        let edit = authoring::apply(
            &t,
            Command::RoundCornerShape {
                panel: panel.id.clone(),
                edge: edge_name.clone(),
                radius_m,
                tension: CIRCLE_TENSION,
                rule: CornerRule::Smooth,
            },
        )
        .unwrap();
        let edited_poly = edit.template.panels[0].polygon.as_ref().unwrap();
        assert!(edited_poly.points[corner_idx].distance(prev.start) < 1e-12);
        assert!(edited_poly.points[corner_idx + 1].distance(prev.end) < 1e-12);
        let controls = edited_poly.curves[&edited_poly.edges[corner_idx]];
        assert!(controls[0].distance(prev.control_1) < 1e-12);
        assert!(controls[1].distance(prev.control_2) < 1e-12);
        edit.template.compile().unwrap();
    }

    // Straight preview helper agrees
    assert!(
        preview_fillet(
            &t.panels[0],
            &t.panels[0].polygon.as_ref().unwrap().edges[2],
            0.015,
            CIRCLE_TENSION
        )
        .is_ok()
    );
}

#[test]
fn tshirt_unsewn_neckline_corners_round_while_sewn_corner_blocks() {
    use marvelous_cloth::authoring::{
        CornerRule,
        corner_tool::{CIRCLE_TENSION, validate_fillet_preview},
    };
    use marvelous_cloth::garment::presets::{self, GarmentKind};

    let t = presets::basic(GarmentKind::Tshirt, 0);
    // "neck-bottom" meets unsewn "neck-left" -> concave corner should round cleanly
    assert!(validate_fillet_preview(&t, "front", "neck-bottom", 0.008, CIRCLE_TENSION).is_ok());
    let edit = authoring::apply(
        &t,
        Command::RoundCornerShape {
            panel: "front".into(),
            edge: "neck-bottom".into(),
            radius_m: 0.008,
            tension: CIRCLE_TENSION,
            rule: CornerRule::Smooth,
        },
    )
    .unwrap();
    let poly = edit.template.panels[0].polygon.as_ref().unwrap();
    assert!(poly.curves.keys().any(|k| k.starts_with("corner_round_")));
    edit.template.compile().unwrap();

    // "shoulder-right" IS sewn on "front" -> must still refuse rounding
    let blocked = authoring::apply(
        &t,
        Command::RoundCornerShape {
            panel: "front".into(),
            edge: "shoulder-right".into(),
            radius_m: 0.008,
            tension: CIRCLE_TENSION,
            rule: CornerRule::Smooth,
        },
    );
    assert!(blocked.is_err());
}

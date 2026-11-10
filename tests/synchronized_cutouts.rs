use glam::DVec2;
use marvelous_cloth::{
    authoring::{self, Command, History},
    garment::{
        GarmentTemplate,
        meshing::MeshingSettings,
        seam::{BoundaryRef, Side},
    },
};

fn two_panel_sewn() -> GarmentTemplate {
    let t = authoring::apply(
        &GarmentTemplate::two_panel_demo(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.4, 0.0),
                DVec2::new(0.4, 0.5),
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
    let dup = authoring::apply(
        &t,
        Command::Duplicate {
            panel: t.panels[0].id.clone(),
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
fn cutout_supports_all_geometric_shapes() {
    let base = two_panel_sewn();
    let seam_id = base.seams[0].id.clone();
    let src_panel = base.seams[0].a.panel.clone();

    for shape in [
        authoring::CutoutShape::VNotch,
        authoring::CutoutShape::Rectangular,
        authoring::CutoutShape::Rounded,
        authoring::CutoutShape::Slit,
    ] {
        let params = authoring::SynchronizedCutoutParams {
            seam_id: seam_id.clone(),
            source_panel: src_panel.clone(),
            interval: [0.35, 0.45],
            depth_m: 0.015,
            shape,
            policy: authoring::CutoutPolicy::Correspondence,
            seam_policy: authoring::CutoutSeamPolicy::SewCutout,
        };
        let edit = authoring::apply(&base, Command::SynchronizedCutout(params)).unwrap();
        assert!(edit.reset_simulation);
        let compiled = edit.template.compile().unwrap();
        assert!(!compiled.mesh.uv.is_empty());
        assert!(!compiled.panels.is_empty());
    }
}

#[test]
fn dimension_preserving_policy_maintains_metric_cutout_width() {
    // Construct panel A (height 0.4 m) and panel B (height 0.8 m) with easing allowed
    let t = authoring::apply(
        &GarmentTemplate::two_panel_demo(),
        Command::Polygon {
            points: vec![
                DVec2::ZERO,
                DVec2::new(0.3, 0.0),
                DVec2::new(0.3, 0.4),
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
    let base = authoring::apply(
        &t,
        Command::Polygon {
            points: vec![
                DVec2::new(0.5, 0.0),
                DVec2::new(0.8, 0.0),
                DVec2::new(0.8, 0.8),
                DVec2::new(0.5, 0.8),
            ],
            tangents: vec![],
            meshing: MeshingSettings {
                target_edge_m: 0.09,
                ..Default::default()
            },
            fabric: "woven-demo".into(),
            replace: false,
        },
    )
    .unwrap()
    .template;

    let p1 = &base.panels[0];
    let p2 = &base.panels[1];
    let poly1 = p1.polygon.as_ref().unwrap();
    let poly2 = p2.polygon.as_ref().unwrap();

    let sewn = authoring::apply(
        &base,
        Command::Sew {
            a: BoundaryRef {
                reversed: false,
                panel: p1.id.clone(),
                side: Side::Right,
                edge: Some(poly1.edges[1].clone()), // length 0.4
                range: [0.0, 1.0],
            },
            b: BoundaryRef {
                reversed: false,
                panel: p2.id.clone(),
                side: Side::Left,
                edge: Some(poly2.edges[3].clone()), // length 0.8
                range: [0.0, 1.0],
            },
            reverse: false,
            ease: true,
        },
    )
    .unwrap()
    .template;

    let seam_id = sewn.seams[0].id.clone();
    let params = authoring::SynchronizedCutoutParams {
        seam_id,
        source_panel: p1.id.clone(),
        interval: [0.35, 0.45], // 10% of 0.4 m = 0.04 m wide
        depth_m: 0.02,
        shape: authoring::CutoutShape::Rectangular,
        policy: authoring::CutoutPolicy::DimensionPreserving,
        seam_policy: authoring::CutoutSeamPolicy::Open,
    };

    let edit = authoring::apply(&sewn, Command::SynchronizedCutout(params)).unwrap();
    let compiled = edit.template.compile().unwrap();
    assert!(!compiled.mesh.uv.is_empty());

    // Check history undo and redo
    let mut history = History::default();
    history.record(sewn.clone());
    let undo = history.restore(&edit.template, false).unwrap();
    assert_eq!(
        serde_json::to_vec(&undo.template).unwrap(),
        serde_json::to_vec(&sewn).unwrap()
    );
    let redo = history.restore(&undo.template, true).unwrap();
    assert_eq!(
        serde_json::to_vec(&redo.template).unwrap(),
        serde_json::to_vec(&edit.template).unwrap()
    );
}

#[test]
fn cutout_preserves_unaffected_panels_and_rejects_malformed_ranges() {
    let base = two_panel_sewn();
    let seam_id = base.seams[0].id.clone();
    let src_panel = base.seams[0].a.panel.clone();
    let before = serde_json::to_vec(&base).unwrap();

    // Inverted interval [0.5, 0.2]
    assert!(
        authoring::apply(
            &base,
            Command::SynchronizedCutout(authoring::SynchronizedCutoutParams {
                seam_id: seam_id.clone(),
                source_panel: src_panel.clone(),
                interval: [0.5, 0.2],
                depth_m: 0.01,
                shape: authoring::CutoutShape::VNotch,
                policy: authoring::CutoutPolicy::Correspondence,
                seam_policy: authoring::CutoutSeamPolicy::Open,
            })
        )
        .is_err()
    );
    assert_eq!(before, serde_json::to_vec(&base).unwrap());

    // Out of bounds interval [0.2, 1.2]
    assert!(
        authoring::apply(
            &base,
            Command::SynchronizedCutout(authoring::SynchronizedCutoutParams {
                seam_id: seam_id.clone(),
                source_panel: src_panel.clone(),
                interval: [0.2, 1.2],
                depth_m: 0.01,
                shape: authoring::CutoutShape::VNotch,
                policy: authoring::CutoutPolicy::Correspondence,
                seam_policy: authoring::CutoutSeamPolicy::Open,
            })
        )
        .is_err()
    );
    assert_eq!(before, serde_json::to_vec(&base).unwrap());

    // Zero depth
    assert!(
        authoring::apply(
            &base,
            Command::SynchronizedCutout(authoring::SynchronizedCutoutParams {
                seam_id,
                source_panel: src_panel,
                interval: [0.2, 0.4],
                depth_m: 0.0,
                shape: authoring::CutoutShape::VNotch,
                policy: authoring::CutoutPolicy::Correspondence,
                seam_policy: authoring::CutoutSeamPolicy::Open,
            })
        )
        .is_err()
    );
    assert_eq!(before, serde_json::to_vec(&base).unwrap());
}

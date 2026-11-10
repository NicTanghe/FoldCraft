//! Synchronized material cutouts across sewn boundaries and paired linked mirrors.
use super::*;
use crate::garment::seam::Side;
use serde::{Deserialize, Serialize};

/// Geometric shape of a material cutout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoutShape {
    /// Triangular V-notch cut into the panel.
    VNotch,
    /// Rectangular U-cutout with 90-degree corners.
    Rectangular,
    /// Smooth rounded U-cutout using cubic Bézier curvature.
    Rounded,
    /// Thin material slit.
    Slit,
}

/// Sizing policy when sewn boundaries have unequal rest lengths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoutPolicy {
    /// Maps interval endpoints directly through the seam's rest-arclength correspondence.
    Correspondence,
    /// Centers the cut at the corresponding seam station and preserves source metric width in metres.
    DimensionPreserving,
}

/// Seam behavior for the newly created cutout rim edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoutSeamPolicy {
    /// The cutout rims remain open (unsewn).
    Open,
    /// The matching cutout rims are sewn together.
    SewCutout,
}

/// Parameters for a synchronized seam cutout transaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SynchronizedCutoutParams {
    pub seam_id: String,
    pub source_panel: String,
    /// Normalized interval [u0, u1] on the source seam side, 0.0 <= u0 < u1 <= 1.0.
    pub interval: [f64; 2],
    /// Inward cut depth in material metres.
    pub depth_m: f64,
    pub shape: CutoutShape,
    pub policy: CutoutPolicy,
    pub seam_policy: CutoutSeamPolicy,
}

fn interpolate_knots(knots: &[[f64; 2]], u: f64, from: usize) -> f64 {
    let i = knots
        .partition_point(|k| k[from] <= u)
        .saturating_sub(1)
        .min(knots.len() - 2);
    let span = knots[i + 1][from] - knots[i][from];
    if span <= 1e-12 {
        knots[i][1 - from]
    } else {
        let t = (u - knots[i][from]) / span;
        knots[i][1 - from] * (1.0 - t) + knots[i + 1][1 - from] * t
    }
}

pub(super) fn synchronized_cutout(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    params: &SynchronizedCutoutParams,
) -> Result<(), String> {
    let [u0, u1] = params.interval;
    if !u0.is_finite() || !u1.is_finite() || u0 < 0.0 || u1 > 1.0 || u1 - u0 < 1e-6 {
        return Err("Cutout interval must satisfy 0.0 <= u0 < u1 <= 1.0".into());
    }
    if !params.depth_m.is_finite() || params.depth_m <= 1e-6 {
        return Err("Cutout depth must be finite and positive in metres".into());
    }

    // Locate the seam
    let seam = source
        .seams
        .iter()
        .find(|s| s.id == params.seam_id)
        .ok_or_else(|| format!("Seam {} not found", params.seam_id))?;

    // Determine source and target sides
    let (is_source_a, ref_source, ref_target) = if seam.a.panel == params.source_panel {
        (true, &seam.a, &seam.b)
    } else if seam.b.panel == params.source_panel {
        (false, &seam.b, &seam.a)
    } else {
        return Err(format!(
            "Panel {} is not part of seam {}",
            params.source_panel, params.seam_id
        ));
    };

    if !seam.a_tail.is_empty() || !seam.b_tail.is_empty() {
        return Err("Synchronized cutouts on chained seam tails are not supported".into());
    }

    let target_panel_id = ref_target.panel.clone();
    let source_panel_id = params.source_panel.clone();

    // Check preflight: verify both panels exist and have editable contours
    for id in [&source_panel_id, &target_panel_id] {
        let p = source
            .panels
            .iter()
            .find(|p| p.id == *id)
            .ok_or_else(|| format!("Panel {id} not found"))?;
        let poly = p
            .polygon
            .as_ref()
            .ok_or_else(|| format!("Panel {id} has no editable contour"))?;
        if !poly.target_lengths.is_empty()
            || !poly.measurements.is_empty()
            || !p.pin_points.is_empty()
            || source.pin_targets.iter().any(|pt| pt.panel == *id)
            || source.frozen_panels.iter().any(|fp| fp.panel == *id)
        {
            return Err(
                "Remove target lengths, measurements and pins from affected panels before cutout"
                    .into(),
            );
        }
    }

    let source_edge = ref_source
        .edge
        .as_ref()
        .ok_or("Source boundary edge name missing")?
        .clone();
    let target_edge = ref_target
        .edge
        .as_ref()
        .ok_or("Target boundary edge name missing")?
        .clone();

    // Preserve derived layout origins
    for (i, p) in template.panels.iter_mut().enumerate() {
        p.layout_2d = Some(layout(source, i));
    }

    // Build knots table for correspondence
    let mut knots = vec![[0.0, 0.0]];
    for knot in &seam.notches {
        knots.push(*knot);
    }
    knots.push([1.0, 1.0]);

    // Measure edge lengths
    let (poly_source, poly_target) = {
        let ps = source
            .panels
            .iter()
            .find(|p| p.id == source_panel_id)
            .unwrap()
            .polygon
            .as_ref()
            .unwrap();
        let pt = source
            .panels
            .iter()
            .find(|p| p.id == target_panel_id)
            .unwrap()
            .polygon
            .as_ref()
            .unwrap();
        (ps, pt)
    };
    let l_source_edge = edge_length(poly_source, &source_edge)?;
    let l_target_edge = edge_length(poly_target, &target_edge)?;

    let l_source_seam = l_source_edge * (ref_source.range[1] - ref_source.range[0]);
    let l_target_seam = l_target_edge * (ref_target.range[1] - ref_target.range[0]);

    // Compute target seam interval [v0, v1]
    let (v0, v1) = match params.policy {
        CutoutPolicy::Correspondence => {
            let map_u = |u: f64| -> f64 {
                if is_source_a {
                    let v_raw = interpolate_knots(&knots, u, 0);
                    if seam.reverse_b { 1.0 - v_raw } else { v_raw }
                } else {
                    let u_dir = if seam.reverse_b { 1.0 - u } else { u };
                    interpolate_knots(&knots, u_dir, 1)
                }
            };
            let ma = map_u(u0);
            let mb = map_u(u1);
            (ma.min(mb), ma.max(mb))
        }
        CutoutPolicy::DimensionPreserving => {
            let u_mid = (u0 + u1) * 0.5;
            let v_mid = if is_source_a {
                let v_raw = interpolate_knots(&knots, u_mid, 0);
                if seam.reverse_b { 1.0 - v_raw } else { v_raw }
            } else {
                let u_dir = if seam.reverse_b { 1.0 - u_mid } else { u_mid };
                interpolate_knots(&knots, u_dir, 1)
            };
            let source_width_m = (u1 - u0) * l_source_seam;
            let half_v = (source_width_m * 0.5) / l_target_seam;
            if v_mid - half_v < -1e-8 || v_mid + half_v > 1.0 + 1e-8 {
                return Err("Dimension-preserving cutout overflows target seam interval".into());
            }
            (
                (v_mid - half_v).clamp(0.0, 1.0),
                (v_mid + half_v).clamp(0.0, 1.0),
            )
        }
    };

    if v1 - v0 < 1e-6 {
        return Err("Target cutout interval is degenerate".into());
    }

    // Map seam intervals to boundary fractions
    let source_bound_fracs =
        map_seam_interval_to_edge(ref_source.range, ref_source.reversed, [u0, u1]);
    let target_bound_fracs =
        map_seam_interval_to_edge(ref_target.range, ref_target.reversed, [v0, v1]);

    // Cut source panel
    let (source_cut_edges, source_post_edge) = cut_polygon_boundary(
        template,
        &source_panel_id,
        &source_edge,
        source_bound_fracs,
        params.depth_m,
        params.shape,
    )?;

    // Cut target panel
    let (target_cut_edges, target_post_edge) = cut_polygon_boundary(
        template,
        &target_panel_id,
        &target_edge,
        target_bound_fracs,
        params.depth_m,
        params.shape,
    )?;

    // Update seams: split the original seam around the cutout
    let stiffness = seam.stiffness;
    let allow_ease = seam.allow_ease;
    let old_seam_id = seam.id.clone();

    template.seams.retain(|s| s.id != old_seam_id);

    let (cut_a, _cut_b) = if is_source_a {
        ([u0, u1], [v0, v1])
    } else {
        ([v0, v1], [u0, u1])
    };

    let (edge_a_pre, edge_a_post) = if is_source_a {
        (source_edge.clone(), source_post_edge.clone())
    } else {
        (target_edge.clone(), target_post_edge.clone())
    };
    let (edge_b_pre, edge_b_post) = if is_source_a {
        (target_edge.clone(), target_post_edge)
    } else {
        (source_edge.clone(), source_post_edge)
    };

    // Leading seam portion
    if cut_a[0] > 1e-4 {
        let edge_b = if !seam.reverse_b {
            edge_b_pre.clone()
        } else {
            edge_b_post.clone()
        };
        template.seams.push(SeamSpec {
            id: format!("{old_seam_id}_lead"),
            a: BoundaryRef {
                panel: seam.a.panel.clone(),
                side: seam.a.side,
                edge: Some(edge_a_pre),
                reversed: seam.a.reversed,
                range: [0.0, 1.0],
            },
            b: BoundaryRef {
                panel: seam.b.panel.clone(),
                side: seam.b.side,
                edge: Some(edge_b),
                reversed: seam.b.reversed,
                range: [0.0, 1.0],
            },
            a_tail: vec![],
            b_tail: vec![],
            reverse_b: seam.reverse_b,
            notches: vec![],
            stiffness,
            allow_ease,
        });
    }

    // Trailing seam portion
    if cut_a[1] < 1.0 - 1e-4 {
        let edge_b = if !seam.reverse_b {
            edge_b_post.clone()
        } else {
            edge_b_pre.clone()
        };
        template.seams.push(SeamSpec {
            id: format!("{old_seam_id}_trail"),
            a: BoundaryRef {
                panel: seam.a.panel.clone(),
                side: seam.a.side,
                edge: Some(edge_a_post),
                reversed: seam.a.reversed,
                range: [0.0, 1.0],
            },
            b: BoundaryRef {
                panel: seam.b.panel.clone(),
                side: seam.b.side,
                edge: Some(edge_b),
                reversed: seam.b.reversed,
                range: [0.0, 1.0],
            },
            a_tail: vec![],
            b_tail: vec![],
            reverse_b: seam.reverse_b,
            notches: vec![],
            stiffness,
            allow_ease,
        });
    }

    // If cutout rims should be sewn together
    if params.seam_policy == CutoutSeamPolicy::SewCutout
        && source_cut_edges.len() == target_cut_edges.len()
    {
        for (i, (se_src, se_tgt)) in source_cut_edges.iter().zip(&target_cut_edges).enumerate() {
            template.seams.push(SeamSpec {
                id: format!("{old_seam_id}_rim_{i}"),
                a: BoundaryRef {
                    panel: source_panel_id.clone(),
                    side: Side::Top,
                    edge: Some(se_src.clone()),
                    reversed: false,
                    range: [0.0, 1.0],
                },
                b: BoundaryRef {
                    panel: target_panel_id.clone(),
                    side: Side::Top,
                    edge: Some(se_tgt.clone()),
                    reversed: true,
                    range: [0.0, 1.0],
                },
                a_tail: vec![],
                b_tail: vec![],
                reverse_b: true,
                notches: vec![],
                stiffness,
                allow_ease: true,
            });
        }
    }

    Ok(())
}

fn map_seam_interval_to_edge(seam_range: [f64; 2], reversed: bool, interval: [f64; 2]) -> [f64; 2] {
    let [u0, u1] = interval;
    let span = seam_range[1] - seam_range[0];
    if !reversed {
        [seam_range[0] + u0 * span, seam_range[0] + u1 * span]
    } else {
        [seam_range[1] - u1 * span, seam_range[1] - u0 * span]
    }
}

fn cut_polygon_boundary(
    template: &mut GarmentTemplate,
    panel_id: &str,
    edge_name: &str,
    fractions: [f64; 2],
    depth_m: f64,
    shape: CutoutShape,
) -> Result<(Vec<String>, String), String> {
    let p = template
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or("Panel not found")?;
    let poly = p
        .polygon
        .as_mut()
        .ok_or("This panel has no editable contour")?;
    let i = poly
        .edges
        .iter()
        .position(|e| e == edge_name)
        .ok_or("Boundary not found")?;
    let n = poly.points.len();
    if n < 3 || poly.edges.len() != n {
        return Err("Invalid contour".into());
    }

    let p_start = poly.points[i];
    let p_end = poly.points[(i + 1) % n];

    // Compute boundary points A and B
    let [t0, t1] = fractions;
    if t0 <= 1e-8 || t1 >= 1.0 - 1e-8 || t1 <= t0 + 1e-8 {
        return Err("Cutout must be strictly interior to the boundary".into());
    }

    let is_curved = poly.curves.contains_key(edge_name);
    let (pt_a, pt_b, inward_normal) = if !is_curved {
        let dir = (p_end - p_start).normalize();
        let normal = DVec2::new(-dir.y, dir.x); // Inward CCW normal
        (p_start.lerp(p_end, t0), p_start.lerp(p_end, t1), normal)
    } else {
        let controls = poly.curves[edge_name];
        let cubic_pts = [p_start, controls[0], controls[1], p_end];
        let a = curves::cubic(cubic_pts, t0);
        let b = curves::cubic(cubic_pts, t1);
        let mid_tangent = (b - a).normalize();
        let normal = DVec2::new(-mid_tangent.y, mid_tangent.x);
        (a, b, normal)
    };

    let post_name = unused_edge_name(poly, &format!("{edge_name}_post"));

    let mut cut_edge_names = Vec::new();

    match shape {
        CutoutShape::VNotch => {
            let mid = (pt_a + pt_b) * 0.5;
            let apex = mid + inward_normal * depth_m;
            let name_a = unused_edge_name(poly, &format!("{edge_name}_cut_1"));
            let name_b = unused_edge_name(poly, &format!("{edge_name}_cut_2"));

            poly.points.insert(i + 1, pt_a);
            poly.points.insert(i + 2, apex);
            poly.points.insert(i + 3, pt_b);

            // Original edge i ends at pt_a
            poly.edges.insert(i + 1, name_a.clone());
            poly.edges.insert(i + 2, name_b.clone());
            poly.edges.insert(i + 3, post_name.clone());

            cut_edge_names.push(name_a);
            cut_edge_names.push(name_b);
        }
        CutoutShape::Rectangular => {
            let c1 = pt_a + inward_normal * depth_m;
            let c2 = pt_b + inward_normal * depth_m;
            let name_1 = unused_edge_name(poly, &format!("{edge_name}_cut_1"));
            let name_2 = unused_edge_name(poly, &format!("{edge_name}_cut_2"));
            let name_3 = unused_edge_name(poly, &format!("{edge_name}_cut_3"));

            poly.points.insert(i + 1, pt_a);
            poly.points.insert(i + 2, c1);
            poly.points.insert(i + 3, c2);
            poly.points.insert(i + 4, pt_b);

            poly.edges.insert(i + 1, name_1.clone());
            poly.edges.insert(i + 2, name_2.clone());
            poly.edges.insert(i + 3, name_3.clone());
            poly.edges.insert(i + 4, post_name.clone());

            cut_edge_names.push(name_1);
            cut_edge_names.push(name_2);
            cut_edge_names.push(name_3);
        }
        CutoutShape::Rounded => {
            let name_cut = unused_edge_name(poly, &format!("{edge_name}_cut"));
            let h = depth_m * (4.0 / 3.0);
            let ctrl1 = pt_a + inward_normal * h;
            let ctrl2 = pt_b + inward_normal * h;

            poly.points.insert(i + 1, pt_a);
            poly.points.insert(i + 2, pt_b);

            poly.edges.insert(i + 1, name_cut.clone());
            poly.edges.insert(i + 2, post_name.clone());

            poly.curves.insert(name_cut.clone(), [ctrl1, ctrl2]);
            cut_edge_names.push(name_cut);
        }
        CutoutShape::Slit => {
            let mid = (pt_a + pt_b) * 0.5;
            let dir = if (pt_b - pt_a).length() > 1e-9 {
                (pt_b - pt_a).normalize()
            } else {
                DVec2::X
            };
            let half_w = (pt_b.distance(pt_a) * 0.1).clamp(0.0005, 0.002);
            let s1 = mid - dir * half_w;
            let s2 = mid + dir * half_w;
            let apex = mid + inward_normal * depth_m;
            let name_1 = unused_edge_name(poly, &format!("{edge_name}_slit_1"));
            let name_2 = unused_edge_name(poly, &format!("{edge_name}_slit_2"));

            poly.points.insert(i + 1, s1);
            poly.points.insert(i + 2, apex);
            poly.points.insert(i + 3, s2);

            poly.edges.insert(i + 1, name_1.clone());
            poly.edges.insert(i + 2, name_2.clone());
            poly.edges.insert(i + 3, post_name.clone());

            cut_edge_names.push(name_1);
            cut_edge_names.push(name_2);
        }
    }

    normalize(p);
    Ok((cut_edge_names, post_name))
}

fn unused_edge_name(poly: &PolygonPattern, base: &str) -> String {
    let mut suffix = 1;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !poly.edges.contains(&candidate)
            && !poly.holes.iter().any(|h| h.edges.contains(&candidate))
            && !poly.lines.iter().any(|l| l.id == candidate)
            && !poly.curves.contains_key(&candidate)
        {
            return candidate;
        }
        suffix += 1;
    }
}

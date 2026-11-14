//! Corner rounding and Bézier continuity editing in panel-local rest space.
use super::*;
use serde::{Deserialize, Serialize};

/// Tangent continuity rule governing incident Bézier handles at a corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CornerRule {
    /// Incident handles move independently.
    Sharp,
    /// Opposite collinear handles retain independent lengths (G1 geometric continuity).
    #[default]
    Smooth,
    /// Opposite collinear handles have equal lengths.
    Symmetric,
}

/// Standard cubic Bézier circle-approximation tension factor for a 90-degree corner:
/// 4/3 * tan(pi/8) = 4/3 * (sqrt(2) - 1) ≈ 0.5522847498307936.
pub const CIRCLE_TENSION: f64 = 0.5522847498307936;

pub(super) fn round(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    edge: &str,
    radius: f64,
) -> Result<(), String> {
    round_shape(
        source,
        template,
        panel,
        edge,
        radius,
        CIRCLE_TENSION,
        CornerRule::Smooth,
    )
}

/// Geometric preview for rounding a corner in panel-local material metres.
#[derive(Clone, Debug, PartialEq)]
pub struct FilletPreview {
    pub corner_index: usize,
    pub incoming_edge: String,
    pub outgoing_edge: String,
    pub corner: DVec2,
    pub start: DVec2,
    pub control_1: DVec2,
    pub control_2: DVec2,
    pub end: DVec2,
    pub bisector: DVec2,
    pub turn_radians: f64,
    pub max_radius_m: f64,
}

/// Compute tangent trim points, cubic Bézier controls, and maximum feasible
/// radius for rounding the corner at the start of `edge`, without mutating.
pub fn preview_fillet(
    panel: &Panel,
    edge: &str,
    radius: f64,
    tension: f64,
) -> Result<FilletPreview, String> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err("Radius must be finite and positive".into());
    }
    if !tension.is_finite() || tension <= 0.0 {
        return Err("Tension must be finite and positive".into());
    }
    let poly = panel
        .polygon
        .as_ref()
        .ok_or("This panel has no editable contour")?;
    let i = poly
        .edges
        .iter()
        .position(|e| e == edge)
        .ok_or("Boundary not found")?;
    let n = poly.points.len();
    if n < 3 || poly.edges.len() != n {
        return Err("Invalid contour".into());
    }
    let previous = (i + n - 1) % n;
    if poly.curves.contains_key(&poly.edges[previous]) || poly.curves.contains_key(edge) {
        return Err("Rounding requires two straight incident boundaries".into());
    }
    let corner = poly.points[i];
    let a = corner - poly.points[previous];
    let b = poly.points[(i + 1) % n] - corner;
    let (la, lb) = (a.length(), b.length());
    if !la.is_finite() || !lb.is_finite() || la <= 1e-9 || lb <= 1e-9 {
        return Err("Rounding requires nondegenerate finite boundaries".into());
    }
    let (u, v) = (a / la, b / lb);
    let turn = u.perp_dot(v).atan2(u.dot(v));
    let abs_turn = turn.abs();
    if abs_turn <= 1e-8 || abs_turn >= std::f64::consts::PI - 1e-4 {
        return Err("Rounding requires two non-collinear boundaries".into());
    }
    let half_tan = (abs_turn * 0.5).tan();
    let max_trim = la.min(lb) - 1e-6;
    let max_radius_m = if half_tan > 1e-9 && max_trim > 0.0 {
        max_trim / half_tan
    } else {
        0.0
    };
    let trim = radius * half_tan;
    if !trim.is_finite() || trim <= 1e-9 || trim >= la - 1e-9 || trim >= lb - 1e-9 {
        return Err("Radius is too small or too large for the incident boundaries".into());
    }
    let start = corner - u * trim;
    let end = corner + v * trim;
    let standard_handle = (4.0 / 3.0) * radius * (abs_turn * 0.25).tan();
    let handle = standard_handle * (tension / CIRCLE_TENSION);
    let chord_mid = (start + end) * 0.5;
    let bisector_raw = chord_mid - corner;
    let bisector = if bisector_raw.length_squared() > 1e-18 {
        bisector_raw.normalize()
    } else {
        (-u + v).normalize_or_zero()
    };
    Ok(FilletPreview {
        corner_index: i,
        incoming_edge: poly.edges[previous].clone(),
        outgoing_edge: edge.to_owned(),
        corner,
        start,
        control_1: start + u * handle,
        control_2: end - v * handle,
        end,
        bisector,
        turn_radians: turn,
        max_radius_m,
    })
}

/// Maximum geometric fillet radius (in metres) permitted by the two straight
/// boundaries meeting at the start of `edge`.
pub fn max_fillet_radius(panel: &Panel, edge: &str) -> Result<f64, String> {
    let preview = preview_fillet(panel, edge, 1e-4, CIRCLE_TENSION)?;
    Ok(preview.max_radius_m)
}

/// Endpoint-based reference to a contour corner in panel-local coordinates.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CornerRef {
    pub panel: String,
    pub point: usize,
}

/// Directed boundary connected to a seed boundary via seams or mirror links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectedEdgeRef {
    pub panel: String,
    pub edge: String,
    /// Whether parameter `0 -> 1` on this edge follows the same seam traversal
    /// direction as the seed edge (`true`), or the opposite direction (`false`).
    pub same_direction: bool,
}

fn is_full_range(range: [f64; 2]) -> bool {
    range[0].abs() <= 1e-9 && (range[1] - 1.0).abs() <= 1e-9
}

fn panel_edge_count_and_idx(panel: &Panel, edge: &str) -> Option<(usize, usize)> {
    let poly = panel.polygon.as_ref()?;
    let n = poly.points.len();
    if n < 3 || poly.edges.len() != n {
        return None;
    }
    let idx = poly.edges.iter().position(|e| e == edge)?;
    Some((n, idx))
}

fn seam_side_endpoint_corner(
    template: &GarmentTemplate,
    side_refs: &[&BoundaryRef],
    chain_reversed: bool,
    at_start: bool,
) -> Option<CornerRef> {
    let query_start = at_start ^ chain_reversed;
    let r = if query_start {
        side_refs.first().copied()?
    } else {
        side_refs.last().copied()?
    };
    let edge_name = r.edge.as_deref()?;
    let panel = template.panels.iter().find(|p| p.id == r.panel)?;
    let (n, edge_idx) = panel_edge_count_and_idx(panel, edge_name)?;
    let want_edge_start = query_start ^ r.reversed;
    if want_edge_start {
        if r.range[0].abs() <= 1e-9 {
            Some(CornerRef {
                panel: r.panel.clone(),
                point: edge_idx,
            })
        } else {
            None
        }
    } else if (r.range[1] - 1.0).abs() <= 1e-9 {
        Some(CornerRef {
            panel: r.panel.clone(),
            point: (edge_idx + 1) % n,
        })
    } else {
        None
    }
}

fn collect_connected_corners(
    template: &GarmentTemplate,
    panel: &str,
    point: usize,
    include_mirrors: bool,
) -> Vec<CornerRef> {
    let Some(seed_panel) = template.panels.iter().find(|p| p.id == panel) else {
        return Vec::new();
    };
    let Some(seed_poly) = seed_panel.polygon.as_ref() else {
        return Vec::new();
    };
    if seed_poly.points.len() < 3
        || seed_poly.edges.len() != seed_poly.points.len()
        || point >= seed_poly.points.len()
    {
        return Vec::new();
    }

    let seed = CornerRef {
        panel: panel.to_owned(),
        point,
    };
    let mut visited = std::collections::BTreeSet::new();
    let mut order = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(seed.clone());
    queue.push_back(seed);

    while let Some(cur) = queue.pop_front() {
        order.push(cur.clone());

        for seam in &template.seams {
            let side_a: Vec<&BoundaryRef> = seam.side(false).collect();
            let side_b: Vec<&BoundaryRef> = seam.side(true).collect();
            for at_start in [true, false] {
                let ca = seam_side_endpoint_corner(template, &side_a, false, at_start);
                let cb = seam_side_endpoint_corner(template, &side_b, seam.reverse_b, at_start);
                if let (Some(ca), Some(cb)) = (ca, cb) {
                    if ca == cur && visited.insert(cb.clone()) {
                        queue.push_back(cb.clone());
                    }
                    if cb == cur && visited.insert(ca.clone()) {
                        queue.push_back(ca);
                    }
                }
            }
            // Also check internal 1:1 full-edge matches if chains happen to contain multiple edges
            if side_a.len() == side_b.len() && side_a.len() > 1 {
                for (idx, ra) in side_a.iter().enumerate() {
                    let b_idx = if seam.reverse_b {
                        side_b.len() - 1 - idx
                    } else {
                        idx
                    };
                    let rb = side_b[b_idx];
                    if is_full_range(ra.range)
                        && is_full_range(rb.range)
                        && let (Some(ea), Some(eb)) = (ra.edge.as_deref(), rb.edge.as_deref())
                        && let (Some(pa), Some(pb)) = (
                            template.panels.iter().find(|p| p.id == ra.panel),
                            template.panels.iter().find(|p| p.id == rb.panel),
                        )
                        && let (Some((na, ia)), Some((nb, ib))) = (
                            panel_edge_count_and_idx(pa, ea),
                            panel_edge_count_and_idx(pb, eb),
                        )
                    {
                        let same_dir = !(ra.reversed ^ rb.reversed ^ seam.reverse_b);
                        let pairs = if same_dir {
                            [
                                (
                                    CornerRef {
                                        panel: ra.panel.clone(),
                                        point: ia,
                                    },
                                    CornerRef {
                                        panel: rb.panel.clone(),
                                        point: ib,
                                    },
                                ),
                                (
                                    CornerRef {
                                        panel: ra.panel.clone(),
                                        point: (ia + 1) % na,
                                    },
                                    CornerRef {
                                        panel: rb.panel.clone(),
                                        point: (ib + 1) % nb,
                                    },
                                ),
                            ]
                        } else {
                            [
                                (
                                    CornerRef {
                                        panel: ra.panel.clone(),
                                        point: ia,
                                    },
                                    CornerRef {
                                        panel: rb.panel.clone(),
                                        point: (ib + 1) % nb,
                                    },
                                ),
                                (
                                    CornerRef {
                                        panel: ra.panel.clone(),
                                        point: (ia + 1) % na,
                                    },
                                    CornerRef {
                                        panel: rb.panel.clone(),
                                        point: ib,
                                    },
                                ),
                            ]
                        };
                        for (c0, c1) in pairs {
                            if c0 == cur && visited.insert(c1.clone()) {
                                queue.push_back(c1.clone());
                            }
                            if c1 == cur && visited.insert(c0.clone()) {
                                queue.push_back(c0);
                            }
                        }
                    }
                }
            }
        }

        if include_mirrors
            && let Some(p) = template.panels.iter().find(|p| p.id == cur.panel)
            && let Some(poly) = p.polygon.as_ref()
        {
            let n = poly.points.len();
            let mirror_pt = n - 1 - cur.point;
            for link in &template.mirror_links {
                let partner = if link.a == cur.panel {
                    Some(&link.b)
                } else if link.b == cur.panel {
                    Some(&link.a)
                } else {
                    None
                };
                if let Some(pid) = partner {
                    let next = CornerRef {
                        panel: pid.clone(),
                        point: mirror_pt,
                    };
                    if visited.insert(next.clone()) {
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    order
}

/// Return all corners connected to `(panel, point)` across seams AND mirror links.
pub fn connected_corners(template: &GarmentTemplate, panel: &str, point: usize) -> Vec<CornerRef> {
    collect_connected_corners(template, panel, point, true)
}

/// Return all corners connected to `(panel, point)` across sewing seams.
pub fn connected_seam_corners(
    template: &GarmentTemplate,
    panel: &str,
    point: usize,
) -> Vec<CornerRef> {
    collect_connected_corners(template, panel, point, false)
}

fn collect_connected_edges(
    template: &GarmentTemplate,
    panel: &str,
    edge: &str,
    include_mirrors: bool,
) -> Vec<ConnectedEdgeRef> {
    let Some(seed_panel) = template.panels.iter().find(|p| p.id == panel) else {
        return Vec::new();
    };
    if panel_edge_count_and_idx(seed_panel, edge).is_none() {
        return Vec::new();
    }

    let mut visited = std::collections::BTreeSet::new();
    let mut order = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert((panel.to_owned(), edge.to_owned()));
    queue.push_back(ConnectedEdgeRef {
        panel: panel.to_owned(),
        edge: edge.to_owned(),
        same_direction: true,
    });

    while let Some(cur) = queue.pop_front() {
        order.push(cur.clone());

        for seam in &template.seams {
            if !seam.a_tail.is_empty()
                || !seam.b_tail.is_empty()
                || !is_full_range(seam.a.range)
                || !is_full_range(seam.b.range)
            {
                continue;
            }
            let (Some(ea), Some(eb)) = (seam.a.edge.as_deref(), seam.b.edge.as_deref()) else {
                continue;
            };
            let seam_same_dir = !(seam.a.reversed ^ seam.b.reversed ^ seam.reverse_b);
            if seam.a.panel == cur.panel
                && ea == cur.edge
                && visited.insert((seam.b.panel.clone(), eb.to_owned()))
            {
                queue.push_back(ConnectedEdgeRef {
                    panel: seam.b.panel.clone(),
                    edge: eb.to_owned(),
                    same_direction: !(cur.same_direction ^ seam_same_dir),
                });
            }
            if seam.b.panel == cur.panel
                && eb == cur.edge
                && visited.insert((seam.a.panel.clone(), ea.to_owned()))
            {
                queue.push_back(ConnectedEdgeRef {
                    panel: seam.a.panel.clone(),
                    edge: ea.to_owned(),
                    same_direction: !(cur.same_direction ^ seam_same_dir),
                });
            }
        }

        if include_mirrors {
            for link in &template.mirror_links {
                let partner = if link.a == cur.panel {
                    Some(&link.b)
                } else if link.b == cur.panel {
                    Some(&link.a)
                } else {
                    None
                };
                if let Some(pid) = partner
                    && template
                        .panels
                        .iter()
                        .find(|p| p.id == *pid)
                        .and_then(|p| panel_edge_count_and_idx(p, &cur.edge))
                        .is_some()
                    && visited.insert((pid.clone(), cur.edge.clone()))
                {
                    queue.push_back(ConnectedEdgeRef {
                        panel: pid.clone(),
                        edge: cur.edge.clone(),
                        same_direction: !cur.same_direction,
                    });
                }
            }
        }
    }

    order
}

/// Return all full-boundary edges connected to `(panel, edge)` across seams and mirror links.
pub fn connected_edges(
    template: &GarmentTemplate,
    panel: &str,
    edge: &str,
) -> Vec<ConnectedEdgeRef> {
    collect_connected_edges(template, panel, edge, true)
}

/// Return all full-boundary edges connected to `(panel, edge)` across sewing seams.
pub fn connected_seam_edges(
    template: &GarmentTemplate,
    panel: &str,
    edge: &str,
) -> Vec<ConnectedEdgeRef> {
    collect_connected_edges(template, panel, edge, false)
}

fn mirror_partner<'a>(template: &'a GarmentTemplate, panel: &str) -> Option<&'a str> {
    for link in &template.mirror_links {
        if link.a == panel {
            return Some(link.b.as_str());
        }
        if link.b == panel {
            return Some(link.a.as_str());
        }
    }
    None
}

/// Map a tangent/offset vector at `(src_a0 -> src_a1)` into `(dst_b0 -> dst_b1)`'s
/// orthonormal seam frame `(along_seam, inward_normal)`.
fn map_edge_vector(
    src_a0: DVec2,
    src_a1: DVec2,
    dst_b0: DVec2,
    dst_b1: DVec2,
    same_direction: bool,
    vec: DVec2,
) -> Option<DVec2> {
    let va = src_a1 - src_a0;
    let la = va.length();
    let vb = dst_b1 - dst_b0;
    let lb = vb.length();
    if la <= 1e-12 || lb <= 1e-12 {
        return None;
    }
    let ta = va / la;
    let na = DVec2::new(-ta.y, ta.x);
    let tb = vb / lb;
    let nb = DVec2::new(-tb.y, tb.x);
    let along = vec.dot(ta);
    let inward = vec.dot(na);
    let sb = if same_direction { tb } else { -tb };
    Some(sb * along + nb * inward)
}

fn validate_panel_fillet_metadata(
    source: &GarmentTemplate,
    id: &str,
    allow_full_seams_for_edges: Option<(&str, &str)>,
) -> Result<(), String> {
    let p = source
        .panels
        .iter()
        .find(|p| p.id == id)
        .ok_or("Panel not found")?;
    let poly = p
        .polygon
        .as_ref()
        .ok_or("This panel has no editable contour")?;
    let has_blocking_seam = match allow_full_seams_for_edges {
        Some((prev_edge_name, edge)) => source.seams.iter().any(|s| {
            let touches = s.references().any(|r| {
                r.panel == id
                    && (r.edge.is_none()
                        || r.edge.as_deref() == Some(edge)
                        || r.edge.as_deref() == Some(prev_edge_name))
            });
            if !touches {
                return false;
            }
            // Only single-edge full [0, 1] seams on incident boundaries can be rounded coherently;
            // legacy rectangle seams, multi-segment chains, or partial sub-interval seams block.
            if !s.a_tail.is_empty()
                || !s.b_tail.is_empty()
                || !is_full_range(s.a.range)
                || !is_full_range(s.b.range)
                || s.a.edge.is_none()
                || s.b.edge.is_none()
            {
                return true;
            }
            false
        }),
        None => source.seams.iter().any(|s| {
            let touches = s.references().any(|r| r.panel == id);
            touches
                && (!s.a_tail.is_empty()
                    || !s.b_tail.is_empty()
                    || !is_full_range(s.a.range)
                    || !is_full_range(s.b.range)
                    || s.a.edge.is_none()
                    || s.b.edge.is_none())
        }),
    };
    if has_blocking_seam
        || !poly.target_lengths.is_empty()
        || !poly.seam_allowances.is_empty()
        || !poly.notches.is_empty()
        || !poly.measurements.is_empty()
        || !p.pin_points.is_empty()
        || source.pin_targets.iter().any(|p| p.panel == id)
        || source.frozen_panels.iter().any(|p| p.panel == id)
    {
        return Err(
            "Remove partial/chained seams, targets, markings, measurements and pins before rounding"
                .into(),
        );
    }
    Ok(())
}

/// Validate template rules (seams, connected corners, markings, pins, linked
/// partners) and compute a live fillet preview without mutating state.
pub fn validate_fillet_preview(
    source: &GarmentTemplate,
    panel: &str,
    edge: &str,
    radius: f64,
    tension: f64,
) -> Result<FilletPreview, String> {
    let source_panel = source
        .panels
        .iter()
        .find(|p| p.id == panel)
        .ok_or("Panel not found")?;
    let source_poly = source_panel
        .polygon
        .as_ref()
        .ok_or("This panel has no editable contour")?;
    let edge_pos = source_poly
        .edges
        .iter()
        .position(|e| e == edge)
        .ok_or("Boundary not found")?;
    let source_n = source_poly.points.len();
    if source_n < 3 || source_poly.edges.len() != source_n {
        return Err("Invalid contour".into());
    }

    let seam_corners = connected_seam_corners(source, panel, edge_pos);
    let mut corner_previews = Vec::with_capacity(seam_corners.len());
    let mut min_max_radius = f64::INFINITY;
    let mut mirror_checked = std::collections::BTreeSet::new();

    for c in &seam_corners {
        let cp = source
            .panels
            .iter()
            .find(|p| p.id == c.panel)
            .ok_or("Panel not found")?;
        let cpoly = cp
            .polygon
            .as_ref()
            .ok_or("This panel has no editable contour")?;
        let cn = cpoly.points.len();
        if cn < 3 || cpoly.edges.len() != cn || c.point >= cn {
            return Err("Invalid contour".into());
        }
        let c_out = &cpoly.edges[c.point];
        let c_in = &cpoly.edges[(c.point + cn - 1) % cn];
        validate_panel_fillet_metadata(source, &c.panel, Some((c_in.as_str(), c_out.as_str())))?;
        if let Some(partner) = mirror_partner(source, &c.panel)
            && mirror_checked.insert(partner.to_owned())
        {
            validate_panel_fillet_metadata(source, partner, None)?;
        }
        let prev = preview_fillet(cp, c_out, radius, tension)?;
        min_max_radius = min_max_radius.min(prev.max_radius_m);
        corner_previews.push((c.clone(), prev));
    }

    // Verify that after trimming every incident sewn boundary across connected corners,
    // seam rest lengths remain equal (unless allow_ease is explicitly set).
    let mut edge_trim_sum: std::collections::BTreeMap<(String, String), f64> =
        std::collections::BTreeMap::new();
    for (c, prev) in &corner_previews {
        let trim = radius * (prev.turn_radians.abs() * 0.5).tan();
        *edge_trim_sum
            .entry((c.panel.clone(), prev.incoming_edge.clone()))
            .or_default() += trim;
        *edge_trim_sum
            .entry((c.panel.clone(), prev.outgoing_edge.clone()))
            .or_default() += trim;
    }
    for seam in &source.seams {
        if seam.allow_ease {
            continue;
        }
        let (Some(ea), Some(eb)) = (seam.a.edge.as_deref(), seam.b.edge.as_deref()) else {
            continue;
        };
        let ta = edge_trim_sum
            .get(&(seam.a.panel.clone(), ea.to_owned()))
            .copied()
            .unwrap_or(0.0);
        let tb = edge_trim_sum
            .get(&(seam.b.panel.clone(), eb.to_owned()))
            .copied()
            .unwrap_or(0.0);
        if ta == 0.0 && tb == 0.0 {
            continue;
        }
        let pa = source
            .panels
            .iter()
            .find(|p| p.id == seam.a.panel)
            .and_then(|p| p.polygon.as_ref())
            .ok_or("Panel not found")?;
        let pb = source
            .panels
            .iter()
            .find(|p| p.id == seam.b.panel)
            .and_then(|p| p.polygon.as_ref())
            .ok_or("Panel not found")?;
        let la = edge_length(pa, ea)? - ta;
        let lb = edge_length(pb, eb)? - tb;
        if la <= 1e-6 || lb <= 1e-6 {
            return Err("Radius is too large for incident sewn boundaries".into());
        }
        if (la - lb).abs() > 1e-7 * la.max(lb).max(1e-3) {
            return Err(
                "Connected corners trim sewn edges unevenly; enable ease on seam before rounding"
                    .into(),
            );
        }
    }

    let mut primary = corner_previews
        .into_iter()
        .next()
        .ok_or("Corner not found")?
        .1;
    primary.max_radius_m = min_max_radius;
    Ok(primary)
}

fn find_direct_seam_between(
    seams: &[SeamSpec],
    panel_1: &str,
    edge_1: &str,
    panel_2: &str,
    edge_2: &str,
) -> Option<(bool, f64, bool)> {
    for s in seams {
        if !s.a_tail.is_empty()
            || !s.b_tail.is_empty()
            || !is_full_range(s.a.range)
            || !is_full_range(s.b.range)
        {
            continue;
        }
        let (Some(ea), Some(eb)) = (s.a.edge.as_deref(), s.b.edge.as_deref()) else {
            continue;
        };
        let same_dir = !(s.a.reversed ^ s.b.reversed ^ s.reverse_b);
        if (s.a.panel == panel_1 && ea == edge_1 && s.b.panel == panel_2 && eb == edge_2)
            || (s.b.panel == panel_1 && eb == edge_1 && s.a.panel == panel_2 && ea == edge_2)
        {
            return Some((same_dir, s.stiffness, s.allow_ease));
        }
    }
    None
}

pub(super) fn round_shape(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    edge: &str,
    radius: f64,
    tension: f64,
    _rule: CornerRule,
) -> Result<(), String> {
    let _ = validate_fillet_preview(source, panel, edge, radius, tension)?;

    let edge_pos = source
        .panels
        .iter()
        .find(|p| p.id == panel)
        .and_then(|p| p.polygon.as_ref())
        .and_then(|poly| poly.edges.iter().position(|e| e == edge))
        .ok_or("Boundary not found")?;
    let seam_corners = connected_seam_corners(source, panel, edge_pos);

    // Preserve every derived board origin before any panel's width can change.
    for (i, p) in template.panels.iter_mut().enumerate() {
        p.layout_2d = Some(layout(source, i));
    }

    // Track one representative per mirror_link group (seed panel always takes precedence)
    // so linked::synchronize mirrors the group partner cleanly without conflict.
    let mut representative_by_panel: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let mut edited_reps = std::collections::BTreeSet::new();
    for c in &seam_corners {
        if representative_by_panel.contains_key(&c.panel) {
            continue;
        }
        if let Some(partner) = mirror_partner(source, &c.panel)
            && edited_reps.contains(partner)
        {
            representative_by_panel.insert(c.panel.clone(), partner.to_owned());
        } else {
            edited_reps.insert(c.panel.clone());
            representative_by_panel.insert(c.panel.clone(), c.panel.clone());
        }
    }

    // Group directly-edited corners by panel, sorted descending by corner index so
    // point/edge insertions do not shift earlier indices on the same panel.
    let mut by_panel: std::collections::BTreeMap<String, Vec<usize>> =
        std::collections::BTreeMap::new();
    for c in &seam_corners {
        if representative_by_panel.get(&c.panel) == Some(&c.panel) {
            by_panel.entry(c.panel.clone()).or_default().push(c.point);
        }
    }
    for pts in by_panel.values_mut() {
        pts.sort_unstable();
        pts.dedup();
        pts.reverse();
    }

    struct CreatedCornerRound {
        panel: String,
        point: usize,
        incoming_edge: String,
        outgoing_edge: String,
        round_edge: String,
    }
    let mut created_on_rep: std::collections::BTreeMap<(String, usize), CreatedCornerRound> =
        std::collections::BTreeMap::new();

    for (pid, corner_indices) in &by_panel {
        let p = template
            .panels
            .iter_mut()
            .find(|p| &p.id == pid)
            .ok_or("Panel not found")?;
        let linked = source
            .mirror_links
            .iter()
            .any(|link| &link.a == pid || &link.b == pid);
        let old_width = p.pattern.width;
        let old_height = p.pattern.height;
        let old_translation = p.placement.translation;

        for &corner_idx in corner_indices {
            let out_edge_name = p
                .polygon
                .as_ref()
                .ok_or("This panel has no editable contour")?
                .edges[corner_idx]
                .clone();
            let preview = preview_fillet(p, &out_edge_name, radius, tension)?;
            let poly = p
                .polygon
                .as_mut()
                .ok_or("This panel has no editable contour")?;
            let i = preview.corner_index;
            let mut suffix = 1;
            let name = loop {
                let candidate = format!("corner_round_{suffix}");
                if !poly.edges.contains(&candidate)
                    && !poly.holes.iter().any(|h| h.edges.contains(&candidate))
                    && !poly.lines.iter().any(|l| l.id == candidate)
                {
                    break candidate;
                }
                suffix += 1;
            };
            poly.points[i] = preview.start;
            poly.points.insert(i + 1, preview.end);
            poly.edges[i] = name.clone();
            poly.edges.insert(i + 1, out_edge_name.clone());
            poly.curves
                .insert(name.clone(), [preview.control_1, preview.control_2]);
            created_on_rep.insert(
                (pid.clone(), corner_idx),
                CreatedCornerRound {
                    panel: pid.clone(),
                    point: corner_idx,
                    incoming_edge: preview.incoming_edge,
                    outgoing_edge: out_edge_name,
                    round_edge: name,
                },
            );
        }

        normalize(p);
        // Linked synchronization does not adjust the partner placement when the
        // normalized material frame changes. Refuse instead of moving that piece.
        if linked
            && (p.pattern.width != old_width
                || p.pattern.height != old_height
                || p.placement.translation != old_translation)
        {
            return Err(
                "Linked rounding that changes the material frame is not supported; unlink first"
                    .into(),
            );
        }
    }

    // Build complete map of created corner_round_* edges including mirror-linked panels,
    // using source topology to resolve mirror corner & edge names.
    let mut resolved_corners: Vec<CreatedCornerRound> = Vec::with_capacity(seam_corners.len());
    for c in &seam_corners {
        let rep = &representative_by_panel[&c.panel];
        if rep == &c.panel {
            if let Some(item) = created_on_rep.get(&(c.panel.clone(), c.point)) {
                resolved_corners.push(CreatedCornerRound {
                    panel: item.panel.clone(),
                    point: item.point,
                    incoming_edge: item.incoming_edge.clone(),
                    outgoing_edge: item.outgoing_edge.clone(),
                    round_edge: item.round_edge.clone(),
                });
            }
        } else if let Some(p) = source.panels.iter().find(|p| p.id == c.panel)
            && let Some(poly) = p.polygon.as_ref()
        {
            let n = poly.points.len();
            let rep_pt = n - 1 - c.point;
            if let Some(item) = created_on_rep.get(&(rep.clone(), rep_pt)) {
                resolved_corners.push(CreatedCornerRound {
                    panel: c.panel.clone(),
                    point: c.point,
                    incoming_edge: poly.edges[(c.point + n - 1) % n].clone(),
                    outgoing_edge: poly.edges[c.point].clone(),
                    round_edge: item.round_edge.clone(),
                });
            }
        }
    }

    // If BOTH incident boundaries of a pair of rounded corners were sewn together,
    // stitch their newly created `corner_round_*` arcs so the seam stays closed in 3D.
    let mut added_pairs = std::collections::BTreeSet::new();
    for i in 0..resolved_corners.len() {
        for j in (i + 1)..resolved_corners.len() {
            let ca = &resolved_corners[i];
            let cb = &resolved_corners[j];
            if ca.panel == cb.panel {
                continue;
            }
            let key = if (ca.panel.as_str(), ca.point) <= (cb.panel.as_str(), cb.point) {
                (ca.panel.clone(), ca.point, cb.panel.clone(), cb.point)
            } else {
                (cb.panel.clone(), cb.point, ca.panel.clone(), ca.point)
            };
            if added_pairs.contains(&key) {
                continue;
            }
            let forward_pair = find_direct_seam_between(
                &source.seams,
                &ca.panel,
                &ca.incoming_edge,
                &cb.panel,
                &cb.incoming_edge,
            )
            .zip(find_direct_seam_between(
                &source.seams,
                &ca.panel,
                &ca.outgoing_edge,
                &cb.panel,
                &cb.outgoing_edge,
            ))
            .filter(|((sd1, _, _), (sd2, _, _))| *sd1 && *sd2)
            .map(|((_sd, st1, e1), (_, st2, e2))| (false, 0.5 * (st1 + st2), e1 || e2));

            let reversed_pair = find_direct_seam_between(
                &source.seams,
                &ca.panel,
                &ca.incoming_edge,
                &cb.panel,
                &cb.outgoing_edge,
            )
            .zip(find_direct_seam_between(
                &source.seams,
                &ca.panel,
                &ca.outgoing_edge,
                &cb.panel,
                &cb.incoming_edge,
            ))
            .filter(|((sd1, _, _), (sd2, _, _))| !*sd1 && !*sd2)
            .map(|((_sd, st1, e1), (_, st2, e2))| (true, 0.5 * (st1 + st2), e1 || e2));

            if let Some((reverse_b, stiffness, allow_ease)) = forward_pair.or(reversed_pair) {
                added_pairs.insert(key);
                let seam_id =
                    unused_id("corner_round", template.seams.iter().map(|s| s.id.clone()));
                template.seams.push(SeamSpec {
                    id: seam_id,
                    a: BoundaryRef {
                        reversed: false,
                        panel: ca.panel.clone(),
                        side: crate::garment::seam::Side::Top,
                        edge: Some(ca.round_edge.clone()),
                        range: [0.0, 1.0],
                    },
                    b: BoundaryRef {
                        reversed: false,
                        panel: cb.panel.clone(),
                        side: crate::garment::seam::Side::Top,
                        edge: Some(cb.round_edge.clone()),
                        range: [0.0, 1.0],
                    },
                    a_tail: vec![],
                    b_tail: vec![],
                    reverse_b,
                    notches: vec![],
                    stiffness,
                    allow_ease,
                });
            }
        }
    }

    Ok(())
}

struct SingleHandleOutcome {
    primary_edge: String,
    primary_ctrl: [DVec2; 2],
    neighbor_updated: Option<(String, [DVec2; 2])>,
}

fn apply_single_handle_edit(
    poly: &mut PolygonPattern,
    edge: &str,
    handle_index: usize,
    position: DVec2,
    rule: CornerRule,
) -> Result<SingleHandleOutcome, String> {
    let edge_idx = poly
        .edges
        .iter()
        .position(|e| e == edge)
        .ok_or("Boundary not found")?;
    let n = poly.points.len();
    if n < 3 || poly.edges.len() != n {
        return Err("Invalid contour".into());
    }

    let mut neighbor_updated: Option<(String, [DVec2; 2])> = None;

    if handle_index == 0 {
        let anchor = poly.points[edge_idx];
        let vec = position - anchor;
        let len = vec.length();
        if len <= 1e-9 || !len.is_finite() {
            return Err("Handle length must be non-zero and finite".into());
        }
        let dir = vec / len;

        let controls = poly.curves.entry(edge.to_owned()).or_insert_with(|| {
            let next_pt = poly.points[(edge_idx + 1) % n];
            [
                anchor + (next_pt - anchor) / 3.0,
                anchor + (next_pt - anchor) * (2.0 / 3.0),
            ]
        });
        controls[0] = position;
        let primary_ctrl = *controls;

        let prev_idx = (edge_idx + n - 1) % n;
        let prev_edge = poly.edges[prev_idx].clone();
        match rule {
            CornerRule::Sharp => {}
            CornerRule::Smooth => {
                if let Some(prev_controls) = poly.curves.get_mut(&prev_edge) {
                    let prev_vec = prev_controls[1] - anchor;
                    let prev_len = prev_vec.length().max(1e-6);
                    prev_controls[1] = anchor - dir * prev_len;
                    neighbor_updated = Some((prev_edge, *prev_controls));
                }
            }
            CornerRule::Symmetric => {
                let prev_pt = poly.points[prev_idx];
                let prev_controls = poly.curves.entry(prev_edge.clone()).or_insert_with(|| {
                    [
                        prev_pt + (anchor - prev_pt) / 3.0,
                        anchor + (prev_pt - anchor) / 3.0,
                    ]
                });
                prev_controls[1] = anchor - dir * len;
                neighbor_updated = Some((prev_edge, *prev_controls));
            }
        }
        Ok(SingleHandleOutcome {
            primary_edge: edge.to_owned(),
            primary_ctrl,
            neighbor_updated,
        })
    } else {
        let anchor = poly.points[(edge_idx + 1) % n];
        let vec = position - anchor;
        let len = vec.length();
        if len <= 1e-9 || !len.is_finite() {
            return Err("Handle length must be non-zero and finite".into());
        }
        let dir = vec / len;

        let start_pt = poly.points[edge_idx];
        let controls = poly.curves.entry(edge.to_owned()).or_insert_with(|| {
            [
                start_pt + (anchor - start_pt) / 3.0,
                start_pt + (anchor - start_pt) * (2.0 / 3.0),
            ]
        });
        controls[1] = position;
        let primary_ctrl = *controls;

        let next_idx = (edge_idx + 1) % n;
        let next_edge = poly.edges[next_idx].clone();
        match rule {
            CornerRule::Sharp => {}
            CornerRule::Smooth => {
                if let Some(next_controls) = poly.curves.get_mut(&next_edge) {
                    let next_vec = next_controls[0] - anchor;
                    let next_len = next_vec.length().max(1e-6);
                    next_controls[0] = anchor - dir * next_len;
                    neighbor_updated = Some((next_edge, *next_controls));
                }
            }
            CornerRule::Symmetric => {
                let next_end = poly.points[(next_idx + 1) % n];
                let next_controls = poly.curves.entry(next_edge.clone()).or_insert_with(|| {
                    [
                        anchor + (next_end - anchor) / 3.0,
                        anchor + (next_end - anchor) * (2.0 / 3.0),
                    ]
                });
                next_controls[0] = anchor - dir * len;
                neighbor_updated = Some((next_edge, *next_controls));
            }
        }
        Ok(SingleHandleOutcome {
            primary_edge: edge.to_owned(),
            primary_ctrl,
            neighbor_updated,
        })
    }
}

fn propagate_edge_curve_across_seams(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    seed_panel: &str,
    seed_edge: &str,
    seed_ctrl: [DVec2; 2],
    edited_rep_panels: &mut std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let src_p = source
        .panels
        .iter()
        .find(|p| p.id == seed_panel)
        .ok_or("Panel not found")?;
    let src_poly = src_p.polygon.as_ref().ok_or("Editable contour missing")?;
    let src_idx = src_poly
        .edges
        .iter()
        .position(|e| e == seed_edge)
        .ok_or("Boundary not found")?;
    let src_n = src_poly.points.len();
    let src_a0 = src_poly.points[src_idx];
    let src_a1 = src_poly.points[(src_idx + 1) % src_n];
    let v0 = seed_ctrl[0] - src_a0;
    let v1 = seed_ctrl[1] - src_a1;

    let partners = connected_seam_edges(source, seed_panel, seed_edge);
    for partner in partners.into_iter().skip(1) {
        if let Some(m_partner) = mirror_partner(source, &partner.panel)
            && edited_rep_panels.contains(m_partner)
            && !edited_rep_panels.contains(&partner.panel)
        {
            // Leave mirror-linked partner untouched so linked::synchronize mirrors its partner.
            continue;
        }
        edited_rep_panels.insert(partner.panel.clone());
        let tp = template
            .panels
            .iter_mut()
            .find(|p| p.id == partner.panel)
            .ok_or("Panel not found")?;
        let poly = tp
            .polygon
            .as_mut()
            .ok_or("This panel has no editable contour")?;
        let e_idx = poly
            .edges
            .iter()
            .position(|e| e == &partner.edge)
            .ok_or("Boundary not found")?;
        let n = poly.points.len();
        let b0 = poly.points[e_idx];
        let b1 = poly.points[(e_idx + 1) % n];
        let mv0 = map_edge_vector(src_a0, src_a1, b0, b1, partner.same_direction, v0)
            .ok_or("Degenerate partner boundary")?;
        let mv1 = map_edge_vector(src_a0, src_a1, b0, b1, partner.same_direction, v1)
            .ok_or("Degenerate partner boundary")?;
        let new_ctrl = if partner.same_direction {
            [b0 + mv0, b1 + mv1]
        } else {
            [b0 + mv1, b1 + mv0]
        };
        poly.curves.insert(partner.edge, new_ctrl);
    }
    Ok(())
}

struct AdjustHandleSpec<'a> {
    panel: &'a str,
    edge: &'a str,
    handle_index: usize,
    position: DVec2,
    rule: CornerRule,
    check_mirror_frame: bool,
}

fn adjust_handle_inner(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    spec: AdjustHandleSpec<'_>,
) -> Result<(), String> {
    if !spec.position.is_finite() {
        return Err("Handle position must be finite".into());
    }
    if spec.handle_index > 1 {
        return Err("Handle index must be 0 or 1".into());
    }
    let mut affected = vec![spec.panel];
    for link in &source.mirror_links {
        if link.a == spec.panel {
            affected.push(&link.b);
        }
        if link.b == spec.panel {
            affected.push(&link.a);
        }
    }
    for id in affected {
        let p = source
            .panels
            .iter()
            .find(|p| p.id == id)
            .ok_or("Panel not found")?;
        if p.polygon.is_none() {
            return Err("This panel has no editable contour".into());
        }
    }
    let mirror_frames: Vec<(String, f64, f64, DVec3)> = source
        .panels
        .iter()
        .filter(|p| mirror_partner(source, &p.id).is_some())
        .map(|p| {
            (
                p.id.clone(),
                p.pattern.width,
                p.pattern.height,
                p.placement.translation,
            )
        })
        .collect();
    for (i, p) in template.panels.iter_mut().enumerate() {
        p.layout_2d = Some(layout(source, i));
    }

    let mut edited_rep_panels = std::collections::BTreeSet::new();
    edited_rep_panels.insert(spec.panel.to_owned());

    let outcome = {
        let p = template
            .panels
            .iter_mut()
            .find(|p| p.id == spec.panel)
            .ok_or("Panel not found")?;
        let poly = p
            .polygon
            .as_mut()
            .ok_or("This panel has no editable contour")?;
        apply_single_handle_edit(poly, spec.edge, spec.handle_index, spec.position, spec.rule)?
    };

    // Propagate primary edge curve (and rule-adjusted neighbor edge if modified) across seams
    propagate_edge_curve_across_seams(
        source,
        template,
        spec.panel,
        &outcome.primary_edge,
        outcome.primary_ctrl,
        &mut edited_rep_panels,
    )?;
    if let Some((adj_edge, adj_ctrl)) = outcome.neighbor_updated {
        propagate_edge_curve_across_seams(
            source,
            template,
            spec.panel,
            &adj_edge,
            adj_ctrl,
            &mut edited_rep_panels,
        )?;
    }

    // Also enforce CornerRule on partner corners whose unsewn neighbor edge meets the adjusted seam
    let partner_edges = connected_seam_edges(source, spec.panel, spec.edge);
    for partner in partner_edges.into_iter().skip(1) {
        if !edited_rep_panels.contains(&partner.panel) {
            continue;
        }
        let partner_h = if partner.same_direction {
            spec.handle_index
        } else {
            1 - spec.handle_index
        };
        let tp = template
            .panels
            .iter_mut()
            .find(|p| p.id == partner.panel)
            .ok_or("Panel not found")?;
        let poly = tp
            .polygon
            .as_mut()
            .ok_or("This panel has no editable contour")?;
        if let Some(ctrl) = poly.curves.get(&partner.edge).copied() {
            let p_idx = poly
                .edges
                .iter()
                .position(|e| e == &partner.edge)
                .ok_or("Boundary not found")?;
            let n = poly.points.len();
            let adj_idx = if partner_h == 0 {
                (p_idx + n - 1) % n
            } else {
                (p_idx + 1) % n
            };
            let adj_edge = &poly.edges[adj_idx];
            // Only synthesize/rotate unsewn adjacent edges on partner so we never break
            // another seam that might be attached to that partner corner.
            if !seam_uses(source, &partner.panel, adj_edge) {
                let _ = apply_single_handle_edit(
                    poly,
                    &partner.edge,
                    partner_h,
                    ctrl[partner_h],
                    spec.rule,
                )?;
            }
        }
    }

    for id in &edited_rep_panels {
        let p = template
            .panels
            .iter_mut()
            .find(|p| &p.id == id)
            .ok_or("Panel not found")?;
        normalize(p);
        if spec.check_mirror_frame
            && let Some((_, old_w, old_h, old_t)) = mirror_frames.iter().find(|(mid, ..)| mid == id)
            && (p.pattern.width != *old_w
                || p.pattern.height != *old_h
                || p.placement.translation != *old_t)
        {
            return Err(
                "Linked corner edit that changes the material frame is not supported; unlink first"
                    .into(),
            );
        }
    }

    Ok(())
}

/// Core Bézier handle adjustment logic shared between authoring commit and live 2D drag preview.
pub fn preview_adjust_handle(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    edge: &str,
    handle_index: usize,
    position: DVec2,
    rule: CornerRule,
) -> Result<(), String> {
    adjust_handle_inner(
        source,
        template,
        AdjustHandleSpec {
            panel,
            edge,
            handle_index,
            position,
            rule,
            check_mirror_frame: false,
        },
    )
}

/// Adjust a Bézier control handle at a corner and enforce the chosen continuity rule.
pub(super) fn adjust_handle(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    edge: &str,
    handle_index: usize,
    position: DVec2,
    rule: CornerRule,
) -> Result<(), String> {
    adjust_handle_inner(
        source,
        template,
        AdjustHandleSpec {
            panel,
            edge,
            handle_index,
            position,
            rule,
            check_mirror_frame: true,
        },
    )
}

/// Synchronized corner move across all seam-connected corners, shared by
/// `Command::Anchor` and live 2D corner dragging.
pub fn preview_move_corner(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    anchor: usize,
    position: DVec2,
) -> Result<(), String> {
    if !position.is_finite() {
        return Err("Point coordinates must be finite".into());
    }
    let old_pt = {
        let p = source
            .panels
            .iter()
            .find(|p| p.id == panel)
            .ok_or("Panel not found")?;
        let poly = p
            .polygon
            .as_ref()
            .ok_or("This panel has no editable contour")?;
        if anchor >= poly.points.len() {
            return Err("Point index out of range".into());
        }
        poly.points[anchor]
    };
    let seed_delta = position - old_pt;

    // Fast path for isolated/unsewn corner: bit-for-bit exact assignment via remap_anchors
    let seam_corners = connected_seam_corners(source, panel, anchor);
    if seam_corners.len() <= 1 {
        let p = template
            .panels
            .iter_mut()
            .find(|p| p.id == panel)
            .ok_or("Panel not found")?;
        remap_anchors(p, &[anchor], |_| position)?;
        normalize(p);
        return Ok(());
    }

    // Preserve board origins when multi-panel normalized origins might shift
    for (i, p) in template.panels.iter_mut().enumerate() {
        p.layout_2d = Some(layout(source, i));
    }

    // BFS propagation of corner delta in seam frames so connected sewn edge lengths stay equal.
    let mut corner_deltas: std::collections::BTreeMap<CornerRef, DVec2> =
        std::collections::BTreeMap::new();
    let seed_ref = CornerRef {
        panel: panel.to_owned(),
        point: anchor,
    };
    corner_deltas.insert(seed_ref.clone(), seed_delta);
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(seed_ref.clone());

    while let Some(cur) = queue.pop_front() {
        let cur_delta = corner_deltas[&cur];
        let Some(cp) = source.panels.iter().find(|p| p.id == cur.panel) else {
            continue;
        };
        let Some(cpoly) = cp.polygon.as_ref() else {
            continue;
        };
        let cn = cpoly.points.len();
        let prev_idx = (cur.point + cn - 1) % cn;
        let incident_edges = [
            cpoly.edges[prev_idx].clone(),
            cpoly.edges[cur.point].clone(),
        ];

        for cur_edge in incident_edges {
            let e_idx = cpoly.edges.iter().position(|e| e == &cur_edge).unwrap();
            let a0 = cpoly.points[e_idx];
            let a1 = cpoly.points[(e_idx + 1) % cn];
            for partner in connected_seam_edges(source, &cur.panel, &cur_edge)
                .into_iter()
                .skip(1)
            {
                let Some(pp) = source.panels.iter().find(|p| p.id == partner.panel) else {
                    continue;
                };
                let Some(ppoly) = pp.polygon.as_ref() else {
                    continue;
                };
                let pn = ppoly.points.len();
                let Some(pe_idx) = ppoly.edges.iter().position(|e| e == &partner.edge) else {
                    continue;
                };
                let b0 = ppoly.points[pe_idx];
                let b1 = ppoly.points[(pe_idx + 1) % pn];
                let at_edge_start = cur.point == e_idx;
                let partner_pt = if at_edge_start ^ (!partner.same_direction) {
                    pe_idx
                } else {
                    (pe_idx + 1) % pn
                };
                let target = CornerRef {
                    panel: partner.panel.clone(),
                    point: partner_pt,
                };
                if corner_deltas.contains_key(&target) {
                    continue;
                }
                if let Some(mapped_delta) =
                    map_edge_vector(a0, a1, b0, b1, partner.same_direction, cur_delta)
                {
                    corner_deltas.insert(target.clone(), mapped_delta);
                    queue.push_back(target);
                }
            }
        }
    }

    // Group by panel, skipping mirror-link siblings of already-edited representatives
    let mut edited_reps = std::collections::BTreeSet::new();
    edited_reps.insert(panel.to_owned());
    let mut by_panel: std::collections::BTreeMap<String, Vec<(usize, DVec2)>> =
        std::collections::BTreeMap::new();
    for (cref, delta) in &corner_deltas {
        if cref.panel == panel {
            by_panel
                .entry(cref.panel.clone())
                .or_default()
                .push((cref.point, *delta));
            continue;
        }
        if let Some(m_partner) = mirror_partner(source, &cref.panel)
            && edited_reps.contains(m_partner)
            && !edited_reps.contains(&cref.panel)
        {
            continue;
        }
        edited_reps.insert(cref.panel.clone());
        by_panel
            .entry(cref.panel.clone())
            .or_default()
            .push((cref.point, *delta));
    }

    for (pid, moves) in by_panel {
        let p = template
            .panels
            .iter_mut()
            .find(|p| p.id == pid)
            .ok_or("Panel not found")?;
        for (pt_idx, delta) in moves {
            if pid == panel && pt_idx == anchor {
                remap_anchors(p, &[pt_idx], |_| position)?;
            } else {
                remap_anchors(p, &[pt_idx], |pt| pt + delta)?;
            }
        }
        normalize(p);
    }
    Ok(())
}

pub(super) fn curve_point_connected(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    anchor: usize,
    curved: bool,
) -> Result<(), String> {
    {
        let p = source
            .panels
            .iter()
            .find(|p| p.id == panel)
            .ok_or("Panel not found")?;
        let poly = p
            .polygon
            .as_ref()
            .ok_or("This panel has no editable contour")?;
        if anchor >= poly.points.len() {
            return Err("Point index out of range".into());
        }
    }

    let seam_corners = connected_seam_corners(source, panel, anchor);
    if seam_corners.len() > 1 {
        for (i, p) in template.panels.iter_mut().enumerate() {
            p.layout_2d = Some(layout(source, i));
        }
    }
    let mut edited_reps = std::collections::BTreeSet::new();
    edited_reps.insert(panel.to_owned());

    let mut edges_to_toggle: Vec<(String, String)> = Vec::new();
    let mut visited_edges = std::collections::BTreeSet::new();

    for c in &seam_corners {
        let Some(p) = source.panels.iter().find(|p| p.id == c.panel) else {
            continue;
        };
        let Some(poly) = p.polygon.as_ref() else {
            continue;
        };
        let n = poly.points.len();
        let prev = (c.point + n - 1) % n;
        for edge_idx in [c.point, prev] {
            let ename = &poly.edges[edge_idx];
            for ce in connected_seam_edges(source, &c.panel, ename) {
                if visited_edges.insert((ce.panel.clone(), ce.edge.clone())) {
                    edges_to_toggle.push((ce.panel, ce.edge));
                }
            }
        }
    }

    let mut touched_panels = std::collections::BTreeSet::new();
    for (pid, ename) in edges_to_toggle {
        if pid != panel
            && let Some(m_partner) = mirror_partner(source, &pid)
            && edited_reps.contains(m_partner)
            && !edited_reps.contains(&pid)
        {
            continue;
        }
        edited_reps.insert(pid.clone());
        touched_panels.insert(pid.clone());
        let p = template
            .panels
            .iter_mut()
            .find(|p| p.id == pid)
            .ok_or("Panel not found")?;
        let poly = p
            .polygon
            .as_mut()
            .ok_or("This panel has no editable contour")?;
        let count = poly.points.len();
        let Some(start) = poly.edges.iter().position(|e| e == &ename) else {
            continue;
        };
        if curved {
            if !poly.curves.contains_key(&ename) {
                let a = poly.points[start];
                let b = poly.points[(start + 1) % count];
                poly.curves
                    .insert(ename, [a.lerp(b, 1. / 3.), a.lerp(b, 2. / 3.)]);
            }
        } else {
            poly.curves.remove(&ename);
        }
    }

    for pid in touched_panels {
        let p = template
            .panels
            .iter_mut()
            .find(|p| p.id == pid)
            .ok_or("Panel not found")?;
        normalize(p);
    }
    Ok(())
}

pub(super) fn move_points_connected(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    points: &[usize],
    delta: DVec2,
) -> Result<(), String> {
    if !delta.is_finite() {
        return Err("Move delta must be finite".into());
    }
    let any_sewn = points
        .iter()
        .any(|&pt| connected_seam_corners(source, panel, pt).len() > 1);
    if !any_sewn {
        let p = template
            .panels
            .iter_mut()
            .find(|p| p.id == panel)
            .ok_or("Panel not found")?;
        remap_anchors(p, points, |point| point + delta)?;
        normalize(p);
        return Ok(());
    }

    for (i, p) in template.panels.iter_mut().enumerate() {
        p.layout_2d = Some(layout(source, i));
    }

    let count = source
        .panels
        .iter()
        .find(|p| p.id == panel)
        .ok_or("Panel not found")?
        .polygon
        .as_ref()
        .ok_or("This panel has no editable contour")?
        .points
        .len();
    let mut unique = points.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.is_empty() {
        return Err("Select at least one point".into());
    }
    if unique.iter().any(|&index| index >= count) {
        return Err("Point index out of range".into());
    }

    let mut corner_deltas: std::collections::BTreeMap<CornerRef, DVec2> =
        std::collections::BTreeMap::new();
    let mut queue = std::collections::VecDeque::new();
    for pt in unique {
        let seed_ref = CornerRef {
            panel: panel.to_owned(),
            point: pt,
        };
        corner_deltas.insert(seed_ref.clone(), delta);
        queue.push_back(seed_ref);
    }

    while let Some(cur) = queue.pop_front() {
        let cur_delta = corner_deltas[&cur];
        let Some(cp) = source.panels.iter().find(|p| p.id == cur.panel) else {
            continue;
        };
        let Some(cpoly) = cp.polygon.as_ref() else {
            continue;
        };
        let cn = cpoly.points.len();
        let prev_idx = (cur.point + cn - 1) % cn;
        let incident_edges = [
            cpoly.edges[prev_idx].clone(),
            cpoly.edges[cur.point].clone(),
        ];

        for cur_edge in incident_edges {
            let e_idx = cpoly.edges.iter().position(|e| e == &cur_edge).unwrap();
            let a0 = cpoly.points[e_idx];
            let a1 = cpoly.points[(e_idx + 1) % cn];
            for partner in connected_seam_edges(source, &cur.panel, &cur_edge)
                .into_iter()
                .skip(1)
            {
                let Some(pp) = source.panels.iter().find(|p| p.id == partner.panel) else {
                    continue;
                };
                let Some(ppoly) = pp.polygon.as_ref() else {
                    continue;
                };
                let pn = ppoly.points.len();
                let Some(pe_idx) = ppoly.edges.iter().position(|e| e == &partner.edge) else {
                    continue;
                };
                let b0 = ppoly.points[pe_idx];
                let b1 = ppoly.points[(pe_idx + 1) % pn];
                let at_edge_start = cur.point == e_idx;
                let partner_pt = if at_edge_start ^ (!partner.same_direction) {
                    pe_idx
                } else {
                    (pe_idx + 1) % pn
                };
                let target = CornerRef {
                    panel: partner.panel.clone(),
                    point: partner_pt,
                };
                if corner_deltas.contains_key(&target) {
                    continue;
                }
                if let Some(mapped_delta) =
                    map_edge_vector(a0, a1, b0, b1, partner.same_direction, cur_delta)
                {
                    corner_deltas.insert(target.clone(), mapped_delta);
                    queue.push_back(target);
                }
            }
        }
    }

    let mut edited_reps = std::collections::BTreeSet::new();
    edited_reps.insert(panel.to_owned());
    let mut by_panel: std::collections::BTreeMap<String, Vec<(usize, DVec2)>> =
        std::collections::BTreeMap::new();
    for (cref, d) in &corner_deltas {
        if cref.panel == panel {
            by_panel
                .entry(cref.panel.clone())
                .or_default()
                .push((cref.point, *d));
            continue;
        }
        if let Some(m_partner) = mirror_partner(source, &cref.panel)
            && edited_reps.contains(m_partner)
            && !edited_reps.contains(&cref.panel)
        {
            continue;
        }
        edited_reps.insert(cref.panel.clone());
        by_panel
            .entry(cref.panel.clone())
            .or_default()
            .push((cref.point, *d));
    }

    for (pid, moves) in by_panel {
        let p = template
            .panels
            .iter_mut()
            .find(|p| p.id == pid)
            .ok_or("Panel not found")?;
        for (pt_idx, d) in moves {
            remap_anchors(p, &[pt_idx], |pt| pt + d)?;
        }
        normalize(p);
    }
    Ok(())
}

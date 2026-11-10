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

pub(super) fn round_shape(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    edge: &str,
    radius: f64,
    tension: f64,
    _rule: CornerRule,
) -> Result<(), String> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err("Radius must be finite and positive".into());
    }
    if !tension.is_finite() || tension <= 0.0 {
        return Err("Tension must be finite and positive".into());
    }
    let mut affected = vec![panel];
    let linked = source
        .mirror_links
        .iter()
        .any(|link| link.a == panel || link.b == panel);
    for link in &source.mirror_links {
        if link.a == panel {
            affected.push(&link.b);
        }
        if link.b == panel {
            affected.push(&link.a);
        }
    }
    for id in affected {
        let p = source
            .panels
            .iter()
            .find(|p| p.id == id)
            .ok_or("Panel not found")?;
        let poly = p
            .polygon
            .as_ref()
            .ok_or("This panel has no editable contour")?;
        if source
            .seams
            .iter()
            .any(|s| s.references().any(|r| r.panel == id))
            || !poly.target_lengths.is_empty()
            || !poly.seam_allowances.is_empty()
            || !poly.notches.is_empty()
            || !poly.measurements.is_empty()
            || !p.pin_points.is_empty()
            || source.pin_targets.iter().any(|p| p.panel == id)
            || source.frozen_panels.iter().any(|p| p.panel == id)
        {
            return Err("Remove seams, targets, markings, measurements and pins from both affected panels before rounding".into());
        }
    }
    // Preserve every derived board origin before the edited width can change.
    for (i, p) in template.panels.iter_mut().enumerate() {
        p.layout_2d = Some(layout(source, i));
    }
    let p = template
        .panels
        .iter_mut()
        .find(|p| p.id == panel)
        .ok_or("Panel not found")?;
    let old_width = p.pattern.width;
    let old_height = p.pattern.height;
    let old_translation = p.placement.translation;
    let poly = p
        .polygon
        .as_mut()
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
    if turn <= 1e-8 || turn > std::f64::consts::FRAC_PI_2 + 1e-10 {
        return Err("Rounding supports convex corners with a turn up to 90 degrees".into());
    }
    let trim = radius * (turn * 0.5).tan();
    if !trim.is_finite() || trim <= 1e-9 || trim >= la - 1e-9 || trim >= lb - 1e-9 {
        return Err("Radius is too small or too large for the incident boundaries".into());
    }
    let start = corner - u * trim;
    let end = corner + v * trim;
    let standard_handle = (4.0 / 3.0) * radius * (turn * 0.25).tan();
    let handle = standard_handle * (tension / CIRCLE_TENSION);
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
    poly.points[i] = start;
    poly.points.insert(i + 1, end);
    poly.edges[i] = name.clone();
    poly.edges.insert(i + 1, edge.to_owned());
    poly.curves
        .insert(name, [start + u * handle, end - v * handle]);
    normalize(p);
    // Linked synchronization does not adjust the partner placement when the
    // normalized material frame changes. Refuse instead of moving that piece.
    if linked
        && (p.pattern.width != old_width
            || p.pattern.height != old_height
            || p.placement.translation != old_translation)
    {
        return Err(
            "Linked rounding that changes the material frame is not supported; unlink first".into(),
        );
    }
    Ok(())
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
    if !position.is_finite() {
        return Err("Handle position must be finite".into());
    }
    if handle_index > 1 {
        return Err("Handle index must be 0 or 1".into());
    }
    // Check affected panels
    let mut affected = vec![panel];
    let linked = source
        .mirror_links
        .iter()
        .any(|link| link.a == panel || link.b == panel);
    for link in &source.mirror_links {
        if link.a == panel {
            affected.push(&link.b);
        }
        if link.b == panel {
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
    for (i, p) in template.panels.iter_mut().enumerate() {
        p.layout_2d = Some(layout(source, i));
    }
    let p = template
        .panels
        .iter_mut()
        .find(|p| p.id == panel)
        .ok_or("Panel not found")?;
    let old_width = p.pattern.width;
    let old_height = p.pattern.height;
    let old_translation = p.placement.translation;
    let poly = p
        .polygon
        .as_mut()
        .ok_or("This panel has no editable contour")?;
    let edge_idx = poly
        .edges
        .iter()
        .position(|e| e == edge)
        .ok_or("Boundary not found")?;
    let n = poly.points.len();
    if n < 3 || poly.edges.len() != n {
        return Err("Invalid contour".into());
    }

    if handle_index == 0 {
        // Handle 0 is outgoing from anchor point poly.points[edge_idx]
        let anchor = poly.points[edge_idx];
        let vec = position - anchor;
        let len = vec.length();
        if len <= 1e-9 || !len.is_finite() {
            return Err("Handle length must be non-zero and finite".into());
        }
        let dir = vec / len;

        // Update the primary curve
        let controls = poly.curves.entry(edge.to_owned()).or_insert_with(|| {
            let next_pt = poly.points[(edge_idx + 1) % n];
            [
                anchor + (next_pt - anchor) / 3.0,
                anchor + (next_pt - anchor) * (2.0 / 3.0),
            ]
        });
        controls[0] = position;

        // Apply rule to incoming curve on previous edge
        let prev_idx = (edge_idx + n - 1) % n;
        let prev_edge = &poly.edges[prev_idx];
        match rule {
            CornerRule::Sharp => {}
            CornerRule::Smooth => {
                if let Some(prev_controls) = poly.curves.get_mut(prev_edge) {
                    let prev_vec = prev_controls[1] - anchor;
                    let prev_len = prev_vec.length().max(1e-6);
                    prev_controls[1] = anchor - dir * prev_len;
                }
            }
            CornerRule::Symmetric => {
                let prev_controls = poly.curves.entry(prev_edge.clone()).or_insert_with(|| {
                    let prev_pt = poly.points[prev_idx];
                    [
                        prev_pt + (anchor - prev_pt) / 3.0,
                        anchor + (prev_pt - anchor) / 3.0,
                    ]
                });
                prev_controls[1] = anchor - dir * len;
            }
        }
    } else {
        // Handle 1 is incoming to anchor point poly.points[(edge_idx + 1) % n]
        let anchor = poly.points[(edge_idx + 1) % n];
        let vec = position - anchor;
        let len = vec.length();
        if len <= 1e-9 || !len.is_finite() {
            return Err("Handle length must be non-zero and finite".into());
        }
        let dir = vec / len;

        // Update primary curve
        let controls = poly.curves.entry(edge.to_owned()).or_insert_with(|| {
            let start_pt = poly.points[edge_idx];
            [
                start_pt + (anchor - start_pt) / 3.0,
                start_pt + (anchor - start_pt) * (2.0 / 3.0),
            ]
        });
        controls[1] = position;

        // Apply rule to outgoing curve on next edge
        let next_idx = (edge_idx + 1) % n;
        let next_edge = &poly.edges[next_idx];
        match rule {
            CornerRule::Sharp => {}
            CornerRule::Smooth => {
                if let Some(next_controls) = poly.curves.get_mut(next_edge) {
                    let next_vec = next_controls[0] - anchor;
                    let next_len = next_vec.length().max(1e-6);
                    next_controls[0] = anchor - dir * next_len;
                }
            }
            CornerRule::Symmetric => {
                let next_controls = poly.curves.entry(next_edge.clone()).or_insert_with(|| {
                    let next_end = poly.points[(next_idx + 1) % n];
                    [
                        anchor + (next_end - anchor) / 3.0,
                        anchor + (next_end - anchor) * (2.0 / 3.0),
                    ]
                });
                next_controls[0] = anchor - dir * len;
            }
        }
    }

    normalize(p);
    if linked
        && (p.pattern.width != old_width
            || p.pattern.height != old_height
            || p.placement.translation != old_translation)
    {
        return Err(
            "Linked corner edit that changes the material frame is not supported; unlink first"
                .into(),
        );
    }
    Ok(())
}

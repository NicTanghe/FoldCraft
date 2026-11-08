//! One-shot fillets in panel-local rest space. The authored source is not retained.
use super::*;

pub(super) fn round(
    source: &GarmentTemplate,
    template: &mut GarmentTemplate,
    panel: &str,
    edge: &str,
    radius: f64,
) -> Result<(), String> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err("Radius must be finite and positive".into());
    }
    let mut affected = vec![panel];
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
    let p = template
        .panels
        .iter_mut()
        .find(|p| p.id == panel)
        .ok_or("Panel not found")?;
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
    let handle = (4.0 / 3.0) * radius * (turn * 0.25).tan();
    let mut suffix = 1;
    let name = loop {
        let candidate = format!("{edge}_round_{suffix}");
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
    Ok(())
}

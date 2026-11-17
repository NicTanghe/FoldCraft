//! Compact, renderer-independent views of a resident document.
//!
//! The summaries name panels, boundaries and seams only. Geometry for the 2D
//! board comes from template_catalog preview paths, which are board-space
//! polylines: no draped position and no renderer coordinate enters them.
use crate::session_client::SessionError;
use crate::template_catalog::Selection;
use glam::DVec2;
use serde_json::{Value, json};

pub fn summarize_document(data: &Value) -> Value {
    json!({
        "documentSession": data.get("documentSession"),
        "revision": data.get("revision"),
        "canUndo": data.get("canUndo"),
        "canRedo": data.get("canRedo"),
        "pendingChange": data.get("pendingChange"),
        "document": summarize_template(data.get("template").unwrap_or(&Value::Null)),
    })
}

pub fn summarize_template(template: &Value) -> Value {
    if template.is_null() {
        return Value::Null;
    }
    let panels: Vec<Value> = template
        .get("panels")
        .and_then(Value::as_array)
        .map(|panels| panels.iter().map(summarize_panel).collect())
        .unwrap_or_default();
    let seams: Vec<Value> = template
        .get("seams")
        .and_then(Value::as_array)
        .map(|seams| seams.iter().map(summarize_seam).collect())
        .unwrap_or_default();
    let fabrics: Vec<Value> = template
        .get("fabrics")
        .and_then(Value::as_array)
        .map(|fabrics| {
            fabrics
                .iter()
                .map(|fabric| json!({"id": fabric.get("id"), "name": fabric.get("name")}))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "name": template.get("name"),
        "format": template.get("format"),
        "version": template.get("version"),
        "units": "metres",
        "fabrics": fabrics,
        "panels": panels,
        "seams": seams,
        "mirror_links": template.get("mirror_links").cloned().unwrap_or(json!([])),
        "counts": {"panels": panels.len(), "seams": seams.len()},
    })
}

fn summarize_panel(panel: &Value) -> Value {
    let polygon = panel.get("polygon");
    let boundaries: Vec<String> = match polygon
        .and_then(|polygon| polygon.get("edges"))
        .and_then(Value::as_array)
    {
        Some(edges) => edges
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        None => ["top", "right", "bottom", "left"]
            .map(str::to_owned)
            .to_vec(),
    };
    json!({
        "id": panel.get("id"),
        "name": panel.get("name"),
        "fabric": panel.get("fabric"),
        "grain_degrees": panel.get("grain_degrees"),
        "boundaries": boundaries,
        "placement": panel.get("placement"),
        "layout_2d": panel.get("layout_2d"),
        "measurements": polygon.and_then(|p| p.get("measurements")).cloned().unwrap_or(json!([])),
        "target_lengths": polygon.and_then(|p| p.get("target_lengths")).cloned().unwrap_or(json!({})),
        "seam_allowances": polygon.and_then(|p| p.get("seam_allowances")).cloned().unwrap_or(json!({})),
        "pins": panel.get("pins").cloned().unwrap_or(json!([])),
    })
}

fn summarize_seam(seam: &Value) -> Value {
    json!({
        "id": seam.get("id"),
        "a": seam.get("a"),
        "b": seam.get("b"),
        "reverse_b": seam.get("reverse_b"),
        "allow_ease": seam.get("allow_ease"),
        "stiffness": seam.get("stiffness"),
    })
}

/// Board-space polylines for the 2D drafting view, in metres.
pub fn preview_paths(template: &Value) -> Result<Value, SessionError> {
    let bytes = serde_json::to_vec(template)
        .map_err(|error| SessionError::new("invalid_document", error.to_string()))?;
    let selection = Selection::external(&bytes, "studio".to_owned())
        .map_err(|error| SessionError::new("invalid_document", error))?;
    let mut paths = Vec::new();
    let mut min = DVec2::splat(f64::INFINITY);
    let mut max = DVec2::splat(f64::NEG_INFINITY);
    for path in &selection.preview {
        let points: Vec<[f64; 2]> = path.points_m.iter().map(|point| [point.x, point.y]).collect();
        for point in &path.points_m {
            min = min.min(*point);
            max = max.max(*point);
        }
        paths.push(json!({
            "panel": path.panel,
            "closed": path.closed,
            "internal": path.internal,
            "points": points,
        }));
    }
    let bounds = if paths.is_empty() {
        json!({"min": [0.0, 0.0], "max": [1.0, 1.0]})
    } else {
        json!({"min": [min.x, min.y], "max": [max.x, max.y]})
    };
    Ok(json!({
        "units": "metres",
        "bounds": bounds,
        "paths": paths,
        "warnings": selection.warnings,
        "panelCount": selection.panel_count,
        "seamCount": selection.seam_count,
    }))
}

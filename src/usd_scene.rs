//! Static USD publication, independent of the desktop and the physical solvers.
//!
//! This bounded USDA writer is a publication boundary, not a USD parser or stage
//! implementation. Editable templates remain the source of semantic references;
//! the future event-thread scene adapter can load the result into cssimpler's
//! native `openusd::usd::Stage`. See `docs/usd-scene.md` for losses and units.
use crate::{garment::CompiledGarment, model::State};
use std::{collections::BTreeSet, fmt::Write as _};

fn quoted(value: &str) -> Result<String, String> {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => return Err("USD panel ID contains a control character".into()),
            c => out.push(c),
        }
    }
    out.push('"');
    Ok(out)
}

/// Stable, injective encoding; no sanitization collisions or panel-order indices.
fn prim_name(id: &str) -> String {
    let mut name = String::from("panel_");
    for byte in id.bytes() {
        write!(name, "{byte:02x}").unwrap();
    }
    name
}

fn finite_float(value: f64) -> bool {
    value.is_finite() && (value as f32).is_finite()
}

/// Publish one world-space snapshot as metre-scale, Y-up USDA polygon meshes.
///
/// Each panel becomes a separate Mesh. `points` and `primvars:st` use USD's
/// standard f32 types; `clothcraft:restUV` retains the original f64 material
/// coordinates in metres. Indices are local transient mesh topology, never
/// persistent authoring references. Winding is preserved with `rightHanded`
/// orientation, two-sided cloth, and explicitly disabled subdivision.
///
/// Rejects inconsistent panel partitions, out-of-panel triangles, duplicate IDs,
/// non-finite data and values outside f32 range before returning any artifact.
/// No solver, filesystem, GPU, material network or editable-document state is
/// changed. Velocities, seams, physical properties and other authoring data are
/// deliberately omitted; this is not a project backup or an import adapter.
pub fn garment_stage_usda(garment: &CompiledGarment, state: &State) -> Result<String, String> {
    let mesh = &garment.mesh;
    if state.x.len() != mesh.uv.len() || !state.time.is_finite() {
        return Err("USD snapshot requires matching positions and finite time".into());
    }
    if garment.panels.is_empty() {
        return Err("USD snapshot requires at least one panel".into());
    }
    if state
        .x
        .iter()
        .any(|p| p.to_array().into_iter().any(|v| !finite_float(v)))
        || mesh
            .uv
            .iter()
            .any(|p| p.to_array().into_iter().any(|v| !finite_float(v)))
    {
        return Err("USD points and UVs must be finite and representable as f32".into());
    }
    let mut vertices_seen = vec![false; mesh.uv.len()];
    let mut triangles_seen = vec![false; mesh.triangles.len()];
    let mut ids = BTreeSet::new();
    for panel in &garment.panels {
        if panel.id.is_empty() || !ids.insert(&panel.id) {
            return Err("USD snapshot requires unique nonempty panel IDs".into());
        }
        quoted(&panel.id)?;
        if panel.vertices.start >= panel.vertices.end
            || panel.vertices.end > mesh.uv.len()
            || panel.triangles.start >= panel.triangles.end
            || panel.triangles.end > mesh.triangles.len()
            || panel.vertices.len() > i32::MAX as usize
        {
            return Err(format!("USD panel {} has invalid mesh ranges", panel.id));
        }
        for i in panel.vertices.clone() {
            if std::mem::replace(&mut vertices_seen[i], true) {
                return Err("USD panel vertex ranges overlap".into());
            }
        }
        for i in panel.triangles.clone() {
            if std::mem::replace(&mut triangles_seen[i], true) {
                return Err("USD panel triangle ranges overlap".into());
            }
            let triangle = &mesh.triangles[i];
            if triangle
                .vertices
                .iter()
                .any(|v| !panel.vertices.contains(v))
            {
                return Err(format!(
                    "USD panel {} has an out-of-panel triangle",
                    panel.id
                ));
            }
            let [a, b, c] = triangle.vertices;
            if a == b || b == c || c == a {
                return Err("USD triangle repeats a point index".into());
            }
        }
    }
    if vertices_seen.contains(&false) || triangles_seen.contains(&false) {
        return Err("USD panel ranges must cover the entire mesh".into());
    }

    let mut out = String::from(
        "#usda 1.0\n(\n    defaultPrim = \"World\"\n    metersPerUnit = 1\n    upAxis = \"Y\"\n)\n\ndef Xform \"World\"\n{\n    def Scope \"Garment\"\n    {\n        custom int clothcraft:snapshotVersion = 1\n",
    );
    writeln!(
        out,
        "        custom double clothcraft:timeSeconds = {}",
        state.time
    )
    .unwrap();
    for panel in &garment.panels {
        writeln!(
            out,
            "        def Mesh \"{}\"\n        {{",
            prim_name(&panel.id)
        )
        .unwrap();
        writeln!(
            out,
            "            custom string clothcraft:panelId = {}",
            quoted(&panel.id)?
        )
        .unwrap();
        out.push_str("            uniform token subdivisionScheme = \"none\"\n            uniform token orientation = \"rightHanded\"\n            uniform bool doubleSided = true\n            point3f[] points = [");
        for (j, p) in state.x[panel.vertices.clone()].iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            write!(out, "({}, {}, {})", p.x as f32, p.y as f32, p.z as f32).unwrap();
        }
        out.push_str("]\n            int[] faceVertexCounts = [");
        for j in 0..panel.triangles.len() {
            if j > 0 {
                out.push_str(", ");
            }
            out.push('3');
        }
        out.push_str("]\n            int[] faceVertexIndices = [");
        for (j, tri) in mesh.triangles[panel.triangles.clone()].iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            let [a, b, c] = tri.vertices.map(|v| v - panel.vertices.start);
            write!(out, "{a}, {b}, {c}").unwrap();
        }
        out.push_str("]\n            custom double2[] clothcraft:restUV = [");
        for (j, uv) in mesh.uv[panel.vertices.clone()].iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            write!(out, "({}, {})", uv.x, uv.y).unwrap();
        }
        out.push_str("]\n            texCoord2f[] primvars:st = [");
        for (j, uv) in mesh.uv[panel.vertices.clone()].iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            write!(out, "({}, {})", uv.x as f32, uv.y as f32).unwrap();
        }
        out.push_str("] (\n                interpolation = \"vertex\"\n            )\n            color3f[] primvars:displayColor = [(0.62, 0.68, 0.78)] (\n                interpolation = \"constant\"\n            )\n        }\n");
    }
    out.push_str("    }\n}\n");
    Ok(out)
}

/// Publish a single-mesh model state as metre-scale, Y-up USDA polygon mesh.
pub fn mesh_stage_usda(
    mesh: &crate::model::Mesh,
    state: &crate::model::State,
) -> Result<String, String> {
    if state.x.len() != mesh.uv.len() || !state.time.is_finite() {
        return Err("USD snapshot requires matching positions and finite time".into());
    }
    if mesh.triangles.is_empty() {
        return Err("USD snapshot requires at least one triangle".into());
    }
    if state
        .x
        .iter()
        .any(|p| p.to_array().into_iter().any(|v| !finite_float(v)))
        || mesh
            .uv
            .iter()
            .any(|p| p.to_array().into_iter().any(|v| !finite_float(v)))
    {
        return Err("USD points and UVs must be finite and representable as f32".into());
    }

    let mut out = String::from(
        "#usda 1.0\n(\n    defaultPrim = \"World\"\n    metersPerUnit = 1\n    upAxis = \"Y\"\n)\n\ndef Xform \"World\"\n{\n    def Scope \"Garment\"\n    {\n        custom int clothcraft:snapshotVersion = 1\n",
    );
    writeln!(
        out,
        "        custom double clothcraft:timeSeconds = {}",
        state.time
    )
    .unwrap();
    out.push_str("        def Mesh \"cloth_mesh\"\n        {\n            custom string clothcraft:panelId = \"cloth\"\n            uniform token subdivisionScheme = \"none\"\n            uniform token orientation = \"rightHanded\"\n            uniform bool doubleSided = true\n            point3f[] points = [");
    for (j, p) in state.x.iter().enumerate() {
        if j > 0 {
            out.push_str(", ");
        }
        write!(out, "({}, {}, {})", p.x as f32, p.y as f32, p.z as f32).unwrap();
    }
    out.push_str("]\n            int[] faceVertexCounts = [");
    for j in 0..mesh.triangles.len() {
        if j > 0 {
            out.push_str(", ");
        }
        out.push('3');
    }
    out.push_str("]\n            int[] faceVertexIndices = [");
    for (j, tri) in mesh.triangles.iter().enumerate() {
        if j > 0 {
            out.push_str(", ");
        }
        let [a, b, c] = tri.vertices;
        write!(out, "{a}, {b}, {c}").unwrap();
    }
    out.push_str("]\n            custom double2[] clothcraft:restUV = [");
    for (j, uv) in mesh.uv.iter().enumerate() {
        if j > 0 {
            out.push_str(", ");
        }
        write!(out, "({}, {})", uv.x, uv.y).unwrap();
    }
    out.push_str("]\n            texCoord2f[] primvars:st = [");
    for (j, uv) in mesh.uv.iter().enumerate() {
        if j > 0 {
            out.push_str(", ");
        }
        write!(out, "({}, {})", uv.x as f32, uv.y as f32).unwrap();
    }
    out.push_str("] (\n                interpolation = \"vertex\"\n            )\n            color3f[] primvars:displayColor = [(0.62, 0.68, 0.78)] (\n                interpolation = \"constant\"\n            )\n        }\n    }\n}\n");
    Ok(out)
}

/// Update transient display positions on the event thread, preserving USD identity.
#[cfg(feature = "desktop")]
pub fn update_viewport_points(
    scene: &cssimpler::renderer::gpu::usd::UsdScene,
    path: &str,
    positions: &[glam::DVec3],
) -> Result<(), String> {
    use cssimpler::renderer::gpu::openusd::gf;
    if positions
        .iter()
        .any(|p| p.to_array().into_iter().any(|v| !finite_float(v)))
    {
        return Err("USD display positions must be finite f32 values".into());
    }
    let points: Vec<_> = positions
        .iter()
        .map(|p| gf::vec3f(p.x as f32, p.y as f32, p.z as f32))
        .collect();
    scene
        .stage()
        .attribute(&format!("{path}.points"))
        .map_err(|e| e.to_string())?
        .set(points)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Transient USD mesh path derived from a semantic panel ID.
#[cfg(feature = "desktop")]
pub fn viewport_panel_path(id: &str) -> String {
    format!("/World/Garment/{}", prim_name(id))
}

#[cfg(feature = "desktop")]
pub fn update_viewport_topology(
    scene: &cssimpler::renderer::gpu::usd::UsdScene,
    mesh: &crate::model::Mesh,
) -> Result<(), String> {
    for (name, values) in [
        ("faceVertexCounts", vec![3_i32; mesh.triangles.len()]),
        (
            "faceVertexIndices",
            mesh.triangles
                .iter()
                .flat_map(|t| t.vertices.map(|i| i as i32))
                .collect(),
        ),
    ] {
        scene
            .stage()
            .attribute(&format!("/World/Garment/cloth_mesh.{name}"))
            .map_err(|e| e.to_string())?
            .set(values)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

//! Runtime-only material-space attachments from a garment surface to a rod network.
//!
//! Support triangles describe interpolation only: they do not add shell forces.
//! Each support vertex must correspond to a simulated rod-network point. The
//! caller builds support within one panel and rebuilds it after remeshing.
use glam::{DVec2, DVec3};

const MAX_PAIR_CHECKS: usize = 8_000_000;
const WEIGHT_TOLERANCE: f64 = 1e-10;

/// A support triangle over rod vertices in panel-local rest coordinates.
#[derive(Clone, Copy, Debug)]
pub struct SupportTriangle {
    pub vertices: [usize; 3],
}

/// Transient mapping for a single panel and a specific compiled support mesh.
/// It is deliberately not serializable: its vertex indices become stale when
/// the support topology changes.
#[derive(Clone, Debug)]
pub struct RodSurfaceMapping {
    panel: String,
    vertex_count: usize,
    attachments: Vec<([usize; 3], [f64; 3])>,
}

impl RodSurfaceMapping {
    /// Bind material points to support triangles. Ambiguous overlaps at requested points, uncovered
    /// points and degenerate faces are errors rather than silent extrapolation.
    pub fn bind(
        panel: &str,
        support_uv: &[DVec2],
        faces: &[SupportTriangle],
        material_points: &[DVec2],
    ) -> Result<Self, String> {
        if panel.is_empty() || support_uv.is_empty() || faces.is_empty() {
            return Err("A panel and a nonempty support mesh are required".into());
        }
        if support_uv
            .iter()
            .chain(material_points)
            .any(|p| !p.is_finite())
        {
            return Err("Material coordinates must be finite".into());
        }
        if faces.len().saturating_mul(material_points.len()) > MAX_PAIR_CHECKS {
            return Err("Rod attachment work limit exceeded".into());
        }
        for face in faces {
            let [a, b, c] = face.vertices;
            if a >= support_uv.len()
                || b >= support_uv.len()
                || c >= support_uv.len()
                || a == b
                || b == c
                || c == a
            {
                return Err("Invalid rod support triangle indices".into());
            }
            let ab = support_uv[b] - support_uv[a];
            let ac = support_uv[c] - support_uv[a];
            let scale = ab.length_squared().max(ac.length_squared());
            if !scale.is_finite()
                || scale == 0.0
                || !ab.perp_dot(ac).is_finite()
                || ab.perp_dot(ac).abs() <= 1e-12 * scale
            {
                return Err("Degenerate rod support triangle".into());
            }
        }
        let mut attachments = Vec::with_capacity(material_points.len());
        for &p in material_points {
            let mut found: Option<([usize; 3], [f64; 3])> = None;
            for face in faces {
                let [a, b, c] = face.vertices;
                let ab = support_uv[b] - support_uv[a];
                let ac = support_uv[c] - support_uv[a];
                let ap = p - support_uv[a];
                let det = ab.perp_dot(ac);
                let v = ap.perp_dot(ac) / det;
                let w = ab.perp_dot(ap) / det;
                let mut weights = [1.0 - v - w, v, w];
                if weights.iter().any(|w| {
                    !w.is_finite() || *w < -WEIGHT_TOLERANCE || *w > 1.0 + WEIGHT_TOLERANCE
                }) {
                    continue;
                }
                for w in &mut weights {
                    *w = w.clamp(0.0, 1.0);
                }
                let sum: f64 = weights.iter().sum();
                for w in &mut weights {
                    *w /= sum;
                }
                let candidate = (face.vertices, weights);
                if let Some(previous) = &found {
                    if !equivalent(previous, &candidate) {
                        return Err("Ambiguous overlapping rod support triangles".into());
                    }
                } else {
                    found = Some(candidate);
                }
            }
            attachments
                .push(found.ok_or_else(|| "Material point is outside rod support".to_string())?);
        }
        Ok(Self {
            panel: panel.to_owned(),
            vertex_count: support_uv.len(),
            attachments,
        })
    }

    pub fn panel(&self) -> &str {
        &self.panel
    }

    /// Interpolate from the corresponding current rod positions. A different
    /// topology must be rebound even if it happens to have the same length.
    pub fn evaluate(&self, rod_positions: &[DVec3]) -> Result<Vec<DVec3>, String> {
        if rod_positions.len() != self.vertex_count || rod_positions.iter().any(|p| !p.is_finite())
        {
            return Err("Rod positions do not match the attachment support".into());
        }
        self.attachments
            .iter()
            .map(|(ids, weights)| {
                let point = rod_positions[ids[0]] * weights[0]
                    + rod_positions[ids[1]] * weights[1]
                    + rod_positions[ids[2]] * weights[2];
                if point.is_finite() {
                    Ok(point)
                } else {
                    Err("Non-finite mapped position".into())
                }
            })
            .collect()
    }
}

fn equivalent(a: &([usize; 3], [f64; 3]), b: &([usize; 3], [f64; 3])) -> bool {
    a.0.iter().chain(b.0.iter()).all(|id| {
        let coefficient = |value: &([usize; 3], [f64; 3])| {
            value
                .0
                .iter()
                .zip(value.1)
                .filter_map(|(i, w)| (i == id).then_some(w))
                .sum::<f64>()
        };
        (coefficient(a) - coefficient(b)).abs() <= WEIGHT_TOLERANCE
    })
}

/// Attachment of one compiled garment panel to caller-supplied rod support.
/// The exact compiled garment is retained so callers can reject stale caches.
pub struct GarmentRodMapping {
    garment: std::sync::Arc<super::CompiledGarment>,
    panel: String,
    surface: RodSurfaceMapping,
}

impl GarmentRodMapping {
    pub fn bind(
        garment: std::sync::Arc<super::CompiledGarment>,
        panel: &str,
        support_uv: &[DVec2],
        faces: &[SupportTriangle],
    ) -> Result<Self, String> {
        let range = garment
            .panels
            .iter()
            .find(|entry| entry.id == panel)
            .ok_or_else(|| format!("Unknown panel {panel}"))?
            .vertices
            .clone();
        let uv = garment
            .mesh
            .uv
            .get(range)
            .ok_or("Invalid compiled panel vertex range")?;
        let surface = RodSurfaceMapping::bind(panel, support_uv, faces, uv)?;
        Ok(Self {
            garment,
            panel: panel.to_owned(),
            surface,
        })
    }

    /// Check before use; recompiling a garment invalidates the mapping.
    pub fn matches(&self, garment: &std::sync::Arc<super::CompiledGarment>, panel: &str) -> bool {
        self.panel == panel && std::sync::Arc::ptr_eq(&self.garment, garment)
    }

    /// Export each panel vertex's interpolation weights for sewing assembly.
    /// `rod_offset` is the panel support's starting index in the global network.
    pub fn global_weights(
        &self,
        garment: &std::sync::Arc<super::CompiledGarment>,
        rod_offset: usize,
    ) -> Result<Vec<Vec<(usize, f64)>>, String> {
        if !self.matches(garment, &self.panel) {
            return Err("Rod mapping belongs to a different compiled garment".into());
        }
        self.surface
            .attachments
            .iter()
            .map(|(ids, weights)| {
                ids.iter()
                    .zip(weights)
                    .filter(|(_, weight)| **weight != 0.0)
                    .map(|(id, weight)| {
                        id.checked_add(rod_offset)
                            .map(|global| (global, *weight))
                            .ok_or_else(|| "Rod attachment index overflow".into())
                    })
                    .collect()
            })
            .collect()
    }

    /// Positions are returned in the compiled panel's vertex order.
    pub fn evaluate(
        &self,
        garment: &std::sync::Arc<super::CompiledGarment>,
        rod_positions: &[DVec3],
    ) -> Result<Vec<DVec3>, String> {
        if !self.matches(garment, &self.panel) {
            return Err("Rod mapping belongs to a different compiled garment".into());
        }
        self.surface.evaluate(rod_positions)
    }
}

//! Compile existing garment seam quadrature onto runtime rod attachments.
//! This is independent of rendering and never persists rod vertex indices.
use super::CompiledGarment;
use crate::cosserat::RodStitch;
use std::collections::BTreeMap;

/// One entry per compiled garment vertex. Each entry describes its current
/// panel attachment as convex weights of global rod-network vertex indices.
/// Build this map again whenever either topology is recompiled.
pub fn compile_rod_stitches(
    garment: &CompiledGarment,
    rod_vertex_count: usize,
    garment_vertex_weights: &[Vec<(usize, f64)>],
) -> Result<Vec<RodStitch>, String> {
    if garment_vertex_weights.len() != garment.mesh.uv.len() {
        return Err("Garment-to-rod attachment count does not match garment".into());
    }
    let mut stitches = Vec::new();
    let mut work = 0usize;
    for seam in &garment.seams {
        seam.validate(garment.mesh.uv.len())?;
        for sample in &seam.samples {
            let mut coefficients = BTreeMap::<usize, f64>::new();
            for &(vertex, coefficient) in &sample.point.terms {
                let weights = &garment_vertex_weights[vertex];
                work = work.saturating_add(weights.len());
                if work > 8_000_000 {
                    return Err("Rod stitch compilation work limit exceeded".into());
                }
                let sum: f64 = weights.iter().map(|(_, w)| *w).sum();
                if weights.is_empty()
                    || !sum.is_finite()
                    || (sum - 1.0).abs() > 1e-10
                    || weights.iter().any(|(index, weight)| {
                        *index >= rod_vertex_count || !weight.is_finite() || *weight < 0.0
                    })
                {
                    return Err("Invalid or missing garment-to-rod seam attachment".into());
                }
                for &(index, weight) in weights {
                    *coefficients.entry(index).or_default() += coefficient * weight;
                }
            }
            let terms: Vec<_> = coefficients
                .into_iter()
                .filter(|(_, weight)| *weight != 0.0)
                .collect();
            let stiffness_n_per_m = seam.stiffness * sample.weight_m;
            if !stiffness_n_per_m.is_finite() || stiffness_n_per_m <= 0.0 {
                return Err("Rod stitch stiffness overflow".into());
            }
            // A fully canceled attachment has identically zero gap and energy.
            if !terms.is_empty() {
                stitches.push(RodStitch {
                    terms,
                    stiffness_n_per_m,
                });
            }
        }
    }
    Ok(stitches)
}

//! CPU-only 3×3 crossing-yarn contact probe; not a garment generator.
use glam::DVec3;
use marvelous_cloth::cosserat::{RodContact, RodMaterial, RodModel, RodSettings};

fn main() -> Result<(), String> {
    let count: usize = 3;
    let spacing = 0.02;
    let gap = 0.004;
    let mut positions = Vec::new();
    let mut edges = Vec::new();
    let mut joints = Vec::new();
    let mut pinned = Vec::new();
    for direction in 0..2 {
        for yarn in 0..count {
            let start_vertex = positions.len();
            let start_edge = edges.len();
            for node in 0..count + 2 {
                let crossing = node.saturating_sub(1).min(count - 1);
                let ordinate = (node as f64 - 1.0) * spacing;
                let coordinate = yarn as f64 * spacing;
                let parity = if (yarn + crossing) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                };
                let z = 0.5 * gap * parity * if direction == 0 { 1.0 } else { -1.0 };
                positions.push(if direction == 0 {
                    DVec3::new(coordinate, ordinate, z)
                } else {
                    DVec3::new(ordinate, coordinate, z)
                });
                if node > 0 {
                    edges.push([start_vertex + node - 1, start_vertex + node]);
                    if node > 1 {
                        joints.push([start_edge + node - 2, start_edge + node - 1]);
                    }
                }
            }
            pinned.push(start_vertex);
            pinned.push(start_vertex + count + 1);
        }
    }
    let model = RodModel::new(
        positions,
        edges,
        joints,
        None,
        &pinned,
        &[],
        RodMaterial {
            linear_density: 0.01,
            stretch_shear_rigidity: 5.0,
            bend_twist_rigidity: 2e-5,
        },
    )?;
    let mut state = model.initial_state();
    let settings = RodSettings {
        dt_s: 0.002,
        iterations: 4,
        gravity_m_s2: DVec3::new(0.0, 0.0, -9.81),
        drag_per_s: 1.0,
        contact: Some(RodContact {
            activation_m: 0.006,
            stiffness_j: 1e-5,
            clearance_m: 1e-7,
        }),
        ..RodSettings::default()
    };
    let mut last = model.step(&mut state, settings)?;
    let mut times = vec![last.step_ms];
    for _ in 1..10 {
        last = model.step(&mut state, settings)?;
        times.push(last.step_ms);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "yarns={} vertices={} segments={} steps={} time_s={:.6} min_pair_m={:.9} contact_energy_j={:.9} residual_rms_n={:.9} bounded_updates_last={} rejected_updates_last={} step_median_ms={:.6} step_p95_ms={:.6}",
        count * 2,
        state.positions.len(),
        state.orientations.len(),
        times.len(),
        state.time_s,
        last.minimum_nonadjacent_distance_m.unwrap_or(f64::NAN),
        last.contact_energy_j,
        last.position_residual_rms_n.unwrap_or(f64::NAN),
        last.bounded_updates,
        last.rejected_line_searches,
        times[times.len() / 2],
        times[(times.len() * 95).div_ceil(100) - 1]
    );
    Ok(())
}

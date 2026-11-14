//! Small CPU-only cantilever probe; no graphical window or GPU is used.
use glam::DVec3;
use marvelous_cloth::cosserat::{RodMaterial, RodModel, RodSettings};

fn main() -> Result<(), String> {
    // The topology and material values follow the author's public cantilever
    // example. The stepping and reported residual are described in the docs.
    let n = 10;
    let points = (0..=n)
        .map(|i| DVec3::new(i as f64 * 0.1, 0.0, 0.0))
        .collect();
    let segments = (0..n).map(|i| [i, i + 1]).collect();
    let joints = (0..n - 1).map(|i| [i, i + 1]).collect();
    let model = RodModel::new(
        points,
        segments,
        joints,
        None,
        &[0],
        &[0],
        RodMaterial {
            linear_density: 1.0,
            stretch_shear_rigidity: 100.0,
            bend_twist_rigidity: 4.0,
        },
    )?;
    let mut state = model.initial_state();
    let settings = RodSettings {
        iterations: 16,
        drag_per_s: 3.0,
        ..RodSettings::default()
    };
    let mut times = Vec::new();
    for _ in 0..120 {
        times.push(model.step(&mut state, settings)?.step_ms);
    }
    let last = model.step(&mut state, settings)?;
    times.push(last.step_ms);
    times.sort_by(f64::total_cmp);
    let tip = state.positions[n];
    println!(
        "steps={} time_s={:.6} tip_m=({:.9},{:.9},{:.9}) elastic_energy_j={:.9} residual_rms_n={:.9} max_stretch_shear={:.9} step_median_ms={:.6} step_p95_ms={:.6}",
        times.len(),
        state.time_s,
        tip.x,
        tip.y,
        tip.z,
        last.elastic_energy_j,
        last.position_residual_rms_n.unwrap_or(f64::NAN),
        last.max_stretch_shear,
        times[times.len() / 2],
        times[(times.len() * 95).div_ceil(100) - 1]
    );
    Ok(())
}

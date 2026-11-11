use super::*;
use crate::{
    energy_factors as constraints,
    garment::GarmentTemplate,
    math::{Jet, Scalar},
};

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn gpu_factor_gradients_match_f64_at_identical_rounded_geometry() {
    let (device, queue) = request_device().unwrap();
    let gpu = GpuAvbd::new(device, queue);
    let garment = Arc::new(GarmentTemplate::two_panel_demo().compile().unwrap());
    let mut state = garment.initial_state.clone();
    let solver = VbdSolver::new(garment.clone(), None, &state).unwrap();
    for (i, p) in state.x.iter_mut().enumerate() {
        p.x += 0.01 * (i as f64 * 1.3).sin();
        p.y += 0.004 * (i as f64 * 0.9).cos();
        p.z += 0.005 * (i as f64 * 0.7).sin();
    }
    let r = gpu.allocate(&solver, &state, None, 1024).unwrap();
    let n = state.x.len();
    let positions: Vec<_> = state
        .x
        .iter()
        .map(|p| {
            let p = (*p - r.origin).as_vec3();
            [p.x, p.y, p.z, 0.]
        })
        .collect();
    let particles = vec![
        Particle {
            initial: [0.; 4],
            velocity_mass: [0.; 4],
            target: [0.; 4],
            delta: [0.; 4],
            energy: [0.; 4]
        };
        n
    ];
    gpu.queue
        .write_buffer(&r.buffers[0], 0, bytemuck::cast_slice(&positions));
    gpu.queue
        .write_buffer(&r.buffers[1], 0, bytemuck::cast_slice(&particles));
    let mut config = Vec::<u32>::new();
    config.extend([n as u32, n as u32, 0, 0]);
    config.extend([r.packed.terms.len() as u32, 0, 1024, 16]);
    config.extend(r.packed.roots);
    config.extend(r.packed.offsets);
    config.extend([0; 4]);
    config.extend([0.01f32, 0., 0.003, 100_000.].map(f32::to_bits));
    config.extend([0.; 4].map(f32::to_bits));
    config.extend([0.; 4].map(f32::to_bits));
    gpu.queue
        .write_buffer(&r.buffers[8], 0, bytemuck::cast_slice(&config));
    let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (n * 80) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_bind_group(0, &r.bind, &[]);
        pass.set_pipeline(&gpu.pipelines[5]);
        pass.dispatch_workgroups(n.div_ceil(64) as u32, 1, 1);
        pass.set_pipeline(&gpu.pipelines[15]);
        pass.dispatch_workgroups(n.div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&r.buffers[1], 0, &read, 0, (n * 80) as u64);
    let submission = gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    read.map_async(wgpu::MapMode::Read, .., move |result| {
        tx.send(result).unwrap();
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let data = read.get_mapped_range(..).unwrap();
    let values: &[Particle] = bytemuck::cast_slice(&data);
    let x: Vec<_> = positions
        .iter()
        .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + r.origin)
        .collect();
    let factors = constraints::compile(&garment);
    let mut max_error = 0.0f64;
    for (v, actual) in values.iter().enumerate() {
        let mut g = DVec3::ZERO;
        let mut energy = 0.;
        for c in factors.iter().filter(|c| c.ids.contains(&v)) {
            let p: [[Jet<3>; 3]; 4] = std::array::from_fn(|j| {
                let id = c.ids[j.min(c.ids.len() - 1)];
                std::array::from_fn(|axis| {
                    if id == v {
                        Jet::variable(x[id][axis], axis)
                    } else {
                        Jet::c(x[id][axis])
                    }
                })
            });
            let q = c.value(&garment, &p);
            g += DVec3::from_array(q.g) * (0.1 * c.k * q.v);
            energy += 0.05 * c.k * q.v * q.v / c.ids.len() as f64;
        }
        let actual_g = DVec3::new(
            actual.delta[0] as f64,
            actual.delta[1] as f64,
            actual.delta[2] as f64,
        );
        let error = actual_g.distance(g);
        max_error = max_error.max(error);
        assert!(
            error < 0.001 + 0.001 * g.length(),
            "vertex {v}: {actual_g:?} vs {g:?}, error {error}"
        );
        assert!((actual.delta[3] as f64 - energy).abs() < 1e-5 + 0.001 * energy.abs());
    }
    eprintln!(
        "GPU finite-factor gradient maximum absolute error: {max_error:.6e} N ({n} vertices)"
    );
    drop(data);
    read.unmap();
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn contact_overflow_retries_and_grab_targets_keep_residency() {
    use crate::vbd::Grab;
    let (device, queue) = request_device().unwrap();
    let gpu = GpuAvbd::new(device, queue);
    let mut t = GarmentTemplate::two_panel_demo();
    t.seams.clear();
    for (i, p) in t.panels.iter_mut().enumerate() {
        p.pins.clear();
        p.pattern = crate::model::Pattern {
            width: 0.12,
            height: 0.12,
            nx: 2,
            ny: 2,
        };
        p.placement.rotation_degrees = DVec3::ZERO;
        p.placement.translation = DVec3::new(0., 1., 0.003 * i as f64);
    }
    let g = Arc::new(t.compile().unwrap());
    let mut state = g.initial_state.clone();
    let mut solver = VbdSolver::new(g, None, &state).unwrap();
    let settings = VbdSettings {
        dt: 1. / 60.,
        iterations: 4,
        gravity: DVec3::ZERO,
        ..Default::default()
    };
    *gpu.resident.borrow_mut() = Some(gpu.allocate(&solver, &state, None, 1).unwrap());
    let m = gpu.step(&mut solver, &mut state, &settings, None).unwrap();
    assert!(m.contact_terms > 1);
    assert!(gpu.resident.borrow().as_ref().unwrap().capacity > 1);
    solver.world.validate_start(&state.x).unwrap();
    let mut grab = Grab {
        vertices: [0, 1, 3],
        weights: [0.5, 0.25, 0.25],
        target: state.x[0],
        stiffness: 1.,
        patch: vec![],
    };
    gpu.step(&mut solver, &mut state, &settings, Some(&grab))
        .unwrap();
    let before = gpu.resident.borrow().as_ref().unwrap().buffers[0].clone();
    grab.target.x += 0.0001;
    gpu.step(&mut solver, &mut state, &settings, Some(&grab))
        .unwrap();
    assert_eq!(
        before,
        gpu.resident.borrow().as_ref().unwrap().buffers[0],
        "moving the target must not reallocate/reupload the position buffer"
    );
    let accepted = state.clone();
    grab.weights[0] = f64::NAN;
    assert!(
        gpu.step(&mut solver, &mut state, &settings, Some(&grab))
            .is_err()
    );
    assert_eq!(state.x, accepted.x);
    assert_eq!(state.v, accepted.v);
    assert_eq!(state.time, accepted.time);
}

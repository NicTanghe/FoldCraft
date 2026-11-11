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
            cg: [[0.; 4]; 7],
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
        size: (n * std::mem::size_of::<Particle>()) as u64,
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
    encoder.copy_buffer_to_buffer(
        &r.buffers[1],
        0,
        &read,
        0,
        (n * std::mem::size_of::<Particle>()) as u64,
    );
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
    for method in [ContactMethod::Ipc, ContactMethod::BarrierAl] {
        check_overflow_and_grabs(method);
    }
}
fn check_overflow_and_grabs(method: ContactMethod) {
    use crate::vbd::Grab;
    let (device, queue) = request_device().unwrap();
    let mut gpu = GpuAvbd::new(device, queue);
    gpu.contact_method = method;
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

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn barrier_al_scalar_gpu_matches_f64() {
    let (device, queue) = request_device().unwrap();
    let mut gpu = GpuAvbd::new(device, queue);
    gpu.contact_method = ContactMethod::BarrierAl;
    let garment = Arc::new(GarmentTemplate::two_panel_demo().compile().unwrap());
    let state = garment.initial_state.clone();
    let solver = VbdSolver::new(garment, None, &state).unwrap();
    let r = gpu.allocate(&solver, &state, None, 1024).unwrap();
    let n = state.x.len();
    let radius = 0.003f32;
    let k = 100_000f32;
    let mut particles = Vec::new();
    let mut records = Vec::<u32>::new();
    let mut expected = Vec::new();
    for i in 0..n {
        let d = [0.00003f32, 0.0003, 0.002, 0.004][i % 4];
        let c = crate::barrier_al::Contact::new(d as f64 * 0.91, radius as f64, k as f64, 20000.);
        let c = crate::barrier_al::Contact {
            slack: c.slack as f32 as f64,
            dual: c.dual as f32 as f64,
            rho: c.rho as f32 as f64,
        };
        let value = c.primal(d as f64, radius as f64, k as f64);
        let mut updated = c;
        updated.update(d as f64, radius as f64, k as f64).unwrap();
        expected.push((value, updated));
        records.extend([
            0,
            0,
            0,
            0,
            0,
            (c.slack as f32).to_bits(),
            (c.dual as f32).to_bits(),
            (c.rho as f32).to_bits(),
            0,
            0,
            0,
            0,
        ]);
        records.extend([0u32; 12]);
        particles.push(Particle {
            cg: [[0.; 4]; 7],
            initial: [d, 0., 0., 0.],
            velocity_mass: [0.; 4],
            target: [0.; 4],
            delta: [0.; 4],
            energy: [0.; 4],
        });
    }
    gpu.queue
        .write_buffer(&r.buffers[1], 0, bytemuck::cast_slice(&particles));
    gpu.queue
        .write_buffer(&r.buffers[4], 16, bytemuck::cast_slice(&records));
    let mut config = vec![0u32; 36];
    config[0] = n as u32;
    config[6] = 1024;
    config[22] = radius.to_bits();
    config[23] = k.to_bits();
    config[27] = 1e-8f32.to_bits();
    config[32] = 1;
    gpu.queue
        .write_buffer(&r.buffers[8], 0, bytemuck::cast_slice(&config));
    let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (n * std::mem::size_of::<Particle>() + 64) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_bind_group(0, &r.bind, &[]);
        pass.set_pipeline(&gpu.pipelines[21]);
        pass.dispatch_workgroups(n.div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(
        &r.buffers[1],
        0,
        &read,
        0,
        (n * std::mem::size_of::<Particle>()) as u64,
    );
    encoder.copy_buffer_to_buffer(
        &r.buffers[7],
        0,
        &read,
        (n * std::mem::size_of::<Particle>()) as u64,
        64,
    );
    let submission = gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    read.map_async(wgpu::MapMode::Read, .., move |r| {
        tx.send(r).unwrap();
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let data = read.get_mapped_range(..).unwrap();
    let errors: u32 = bytemuck::pod_read_unaligned(
        &data[n * std::mem::size_of::<Particle>()..n * std::mem::size_of::<Particle>() + 4],
    );
    assert_eq!(errors, 0);
    let result: &[Particle] = bytemuck::cast_slice(&data[..n * std::mem::size_of::<Particle>()]);
    for (p, (value, c)) in result.iter().zip(expected) {
        for (actual, reference) in p.delta[..3].iter().zip(value) {
            assert!(
                (*actual as f64 - reference).abs() < 3e-5 * reference.abs().max(1.),
                "{actual} vs {reference}"
            );
        }
        assert!((p.delta[3] as f64 - c.slack).abs() < 2e-8);
        assert!((p.energy[0] as f64 - c.dual).abs() < 3e-5 * c.dual.abs().max(1.));
    }
    drop(data);
    read.unmap();
}

fn read_test_buffer(gpu: &GpuAvbd, buffer: &wgpu::Buffer, size: u64) -> Vec<u32> {
    let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(buffer, 0, &read, 0, size);
    let submission = gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    read.map_async(wgpu::MapMode::Read, .., move |r| {
        tx.send(r).unwrap();
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let data = read.get_mapped_range(..).unwrap();
    let result = bytemuck::cast_slice::<u8, u32>(&data).to_vec();
    drop(data);
    read.unmap();
    result
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn barrier_al_matches_full_keys_and_retains_unresolved_pairs() {
    let (device, queue) = request_device().unwrap();
    let mut gpu = GpuAvbd::new(device, queue);
    gpu.contact_method = ContactMethod::BarrierAl;
    let mut template = GarmentTemplate::two_panel_demo();
    template.seams.clear();
    for (i, p) in template.panels.iter_mut().enumerate() {
        p.pins.clear();
        p.pattern = crate::model::Pattern {
            width: 0.12,
            height: 0.12,
            nx: 2,
            ny: 2,
        };
        p.placement.rotation_degrees = DVec3::ZERO;
        p.placement.translation = DVec3::new(0., 1., 0.002 * i as f64);
    }
    let garment = Arc::new(template.compile().unwrap());
    let mut state = garment.initial_state.clone();
    let mut solver = VbdSolver::new(garment, None, &state).unwrap();
    let settings = VbdSettings {
        iterations: 1,
        gravity: DVec3::ZERO,
        ..Default::default()
    };
    gpu.step(&mut solver, &mut state, &settings, None).unwrap();
    let residency = gpu.resident.borrow();
    let r = residency.as_ref().unwrap();
    let data = read_test_buffer(&gpu, &r.buffers[4], r.buffers[4].size());
    let count = data[0] as usize;
    let history = &data[4 + r.capacity * 24..4 + (r.capacity + count) * 24];
    let old: Vec<Vec<u32>> = history
        .as_chunks::<24>()
        .0
        .iter()
        .map(|p| p.to_vec())
        .collect();
    let removed = old
        .iter()
        .position(|p| f32::from_bits(p[7]) > 0.)
        .expect("active augmented pair");
    let expected: std::collections::HashMap<Vec<u32>, [u32; 2]> = old
        .iter()
        .filter(|p| f32::from_bits(p[7]) > 0.)
        .map(|p| (p[..5].to_vec(), [p[5], p[6]]))
        .collect();
    assert!(expected.len() > 2, "fixture must exercise shared incidence");
    let mut current = old.clone();
    current.remove(removed);
    let shift = current.len() / 2;
    current.rotate_left(shift);
    for p in &mut current {
        p[5] = f32::NAN.to_bits();
        p[6] = f32::NAN.to_bits();
    }
    gpu.queue.write_buffer(
        &r.buffers[4],
        0,
        bytemuck::cast_slice(&[(count - 1) as u32]),
    );
    gpu.queue
        .write_buffer(&r.buffers[4], 16, bytemuck::cast_slice(&current.concat()));
    let mut control = vec![0u32; 16];
    control[3] = 1;
    control[6] = count as u32;
    gpu.queue
        .write_buffer(&r.buffers[7], 0, bytemuck::cast_slice(&control));
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_bind_group(0, &r.bind, &[]);
        for (entry, groups) in [
            (5, state.x.len().div_ceil(64)),
            (6, r.capacity.div_ceil(64)),
            (16, r.capacity.div_ceil(64)),
            (17, r.capacity.div_ceil(64)),
        ] {
            pass.set_pipeline(&gpu.pipelines[entry]);
            pass.dispatch_workgroups(groups as u32, 1, 1);
        }
    }
    gpu.queue.submit([encoder.finish()]);
    let actual = read_test_buffer(&gpu, &r.buffers[4], r.buffers[4].size());
    assert_eq!(
        actual[0] as usize, count,
        "missing unresolved pair must be retained"
    );
    for p in actual[4..4 + count * 24].as_chunks::<24>().0 {
        if let Some(values) = expected.get(&p[..5]) {
            assert_eq!(
                &p[5..7],
                values,
                "dual state followed insertion order rather than primitive identity"
            );
        }
    }
    let control = read_test_buffer(&gpu, &r.buffers[7], 64);
    assert_eq!(control[0], 0);
    assert!(control[7] > 0);
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn coupled_material_matvec_matches_independent_f64_factor_assembly() {
    let (device, queue) = request_device().unwrap();
    let gpu = GpuAvbd::new(device, queue);
    let mut g = GarmentTemplate::two_panel_demo().compile().unwrap();
    g.mesh.pins.fill(None);
    let garment = Arc::new(g);
    let mut state = garment.initial_state.clone();
    for (i, x) in state.x.iter_mut().enumerate() {
        x.z += 0.003 * (i as f64).sin();
    }
    let solver = VbdSolver::new(garment.clone(), None, &state).unwrap();
    let r = gpu.allocate(&solver, &state, None, 1024).unwrap();
    let n = state.x.len();
    let positions: Vec<_> = state
        .x
        .iter()
        .map(|x| (*x - r.origin).as_vec3().extend(0.).to_array())
        .collect();
    let direction: Vec<_> = (0..n)
        .map(|i| {
            glam::Vec3::new(
                (i as f32).sin(),
                (i as f32 * 0.7).cos(),
                (i as f32 * 1.1).sin(),
            )
        })
        .collect();
    let particles: Vec<_> = direction
        .iter()
        .map(|p| {
            let mut cg = [[0.; 4]; 7];
            cg[2] = p.extend(0.).to_array();
            Particle {
                initial: [0.; 4],
                velocity_mass: [0.; 4],
                target: [0., 0., 0., 1.5],
                delta: [0.; 4],
                energy: [0.; 4],
                cg,
            }
        })
        .collect();
    gpu.queue
        .write_buffer(&r.buffers[0], 0, bytemuck::cast_slice(&positions));
    gpu.queue
        .write_buffer(&r.buffers[1], 0, bytemuck::cast_slice(&particles));
    let mut config = vec![0u32; 40];
    config[0] = n as u32;
    config[1] = n as u32;
    config[4] = r.packed.terms.len() as u32;
    config[6] = 1024;
    config[8..12].copy_from_slice(&r.packed.roots);
    config[12..16].copy_from_slice(&r.packed.offsets);
    config[22] = 0.003f32.to_bits();
    config[23] = 100000f32.to_bits();
    config[27] = 1e-8f32.to_bits();
    config[36] = 8;
    gpu.queue
        .write_buffer(&r.buffers[8], 0, bytemuck::cast_slice(&config));
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_bind_group(0, &r.bind, &[]);
        for (entry, count) in [
            (5, n.div_ceil(64)),
            (24, r.packed.terms.len().div_ceil(64)),
            (26, n.div_ceil(64)),
        ] {
            pass.set_pipeline(&gpu.pipelines[entry]);
            pass.dispatch_workgroups(count as u32, 1, 1);
        }
    }
    gpu.queue.submit([encoder.finish()]);
    let result = read_test_buffer(
        &gpu,
        &r.buffers[1],
        (n * std::mem::size_of::<Particle>()) as u64,
    );
    let actual: &[Particle] = bytemuck::cast_slice(&result);
    let x: Vec<_> = positions
        .iter()
        .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + r.origin)
        .collect();
    let mut expected: Vec<_> = direction.iter().map(|p| 1.5 * p.as_dvec3()).collect();
    for term in constraints::compile(&garment) {
        let mut gradients = Vec::new();
        let mut curvatures = Vec::new();
        for &v in &term.ids {
            let points: [[Jet<3>; 3]; 4] = std::array::from_fn(|j| {
                let id = term.ids[j.min(term.ids.len() - 1)];
                std::array::from_fn(|axis| {
                    if id == v {
                        Jet::variable(x[id][axis], axis)
                    } else {
                        Jet::c(x[id][axis])
                    }
                })
            });
            let c = term.value(&garment, &points);
            gradients.push(DVec3::from_array(c.g));
            curvatures.push(DVec3::ZERO);
        }
        let dotp: f64 = gradients
            .iter()
            .zip(&term.ids)
            .map(|(g, &v)| g.dot(direction[v].as_dvec3()))
            .sum();
        for (j, &v) in term.ids.iter().enumerate() {
            expected[v] +=
                0.1 * term.k * gradients[j] * dotp + curvatures[j] * direction[v].as_dvec3();
        }
    }
    let mut quadratic = 0.;
    for (v, (p, hp)) in actual.iter().zip(expected).enumerate() {
        let value = DVec3::new(p.cg[3][0] as f64, p.cg[3][1] as f64, p.cg[3][2] as f64);
        assert!(
            value.distance(hp) < 0.002 * hp.length().max(1.),
            "H p mismatch at {v}: {value:?} vs {hp:?}"
        );
        quadratic += value.dot(direction[v].as_dvec3());
    }
    assert!(quadratic > 0., "coupled curvature must be positive");
    // A stressed patch's rigid translation has zero material curvature.
    // The coupled operator must retain only inertia in this direction.
    let translation = glam::Vec3::new(0.3, -0.2, 0.5);
    let mut rigid = particles.clone();
    for p in &mut rigid {
        p.cg[2] = translation.extend(0.).to_array();
    }
    gpu.queue
        .write_buffer(&r.buffers[1], 0, bytemuck::cast_slice(&rigid));
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_bind_group(0, &r.bind, &[]);
        pass.set_pipeline(&gpu.pipelines[26]);
        pass.dispatch_workgroups(n.div_ceil(64) as u32, 1, 1);
    }
    gpu.queue.submit([encoder.finish()]);
    let result = read_test_buffer(
        &gpu,
        &r.buffers[1],
        (n * std::mem::size_of::<Particle>()) as u64,
    );
    let values: &[Particle] = bytemuck::cast_slice(&result);
    for p in values {
        let hp = glam::Vec3::new(p.cg[3][0], p.cg[3][1], p.cg[3][2]);
        assert!(
            hp.distance(1.5 * translation) < 0.003,
            "material curvature resisted rigid translation: {hp:?}"
        );
    }
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn coupled_correction_reduces_stressed_patch_objective() {
    let (device, queue) = request_device().unwrap();
    let mut template = GarmentTemplate::two_panel_demo();
    template.panels.truncate(1);
    template.seams.clear();
    template.panels[0].pattern.nx = 4;
    template.panels[0].pattern.ny = 4;
    let garment = Arc::new(template.compile().unwrap());
    let mut initial = garment.initial_state.clone();
    for (i, x) in initial.x.iter_mut().enumerate() {
        if garment.mesh.pins[i].is_none() {
            x.x += 1e-4 * (i as f64 * 1.7).sin();
        }
    }
    let settings = VbdSettings {
        gravity: DVec3::ZERO,
        iterations: 4,
        dt: 1. / 60.,
        ..Default::default()
    };
    let factors = constraints::compile(&garment);
    let mut energies = Vec::new();
    for iterations in [0, 8] {
        let mut gpu = GpuAvbd::new(device.clone(), queue.clone());
        gpu.coupled_iterations = iterations;
        let mut state = initial.clone();
        let mut solver = VbdSolver::new(garment.clone(), None, &state).unwrap();
        gpu.step(&mut solver, &mut state, &settings, None).unwrap();
        let material: f64 = factors
            .iter()
            .map(|c| 0.5 * c.k * c.at(&garment, &state.x).powi(2))
            .sum();
        let inertia: f64 = state
            .x
            .iter()
            .zip(&initial.x)
            .zip(&garment.mesh.masses)
            .map(|((a, b), m)| {
                0.5 * m * (1. + settings.dt * settings.damping) / settings.dt.powi(2)
                    * a.distance_squared(*b)
            })
            .sum();
        energies.push(material + inertia);
    }
    eprintln!(
        "patch implicit objective: Jacobi={} coupled={}",
        energies[0], energies[1]
    );
    assert!(
        energies[1] < 0.8 * energies[0],
        "coupled solve did not improve physical objective"
    );
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn cached_contact_matvec_matches_independent_vf_and_ee_operators() {
    let (device, queue) = request_device().unwrap();
    for method in [ContactMethod::Ipc, ContactMethod::BarrierAl] {
        let mut gpu = GpuAvbd::new(device.clone(), queue.clone());
        gpu.contact_method = method;
        let garment = Arc::new(GarmentTemplate::two_panel_demo().compile().unwrap());
        let state = garment.initial_state.clone();
        let solver = VbdSolver::new(garment.clone(), None, &state).unwrap();
        let r = gpu.allocate(&solver, &state, None, 1024).unwrap();
        let n = state.x.len();
        // Isolate contact from all material rows. Both VF and EE witnesses lie
        // strictly inside their primitives; the known barycentrics are analytic.
        gpu.queue.write_buffer(
            &r.buffers[3],
            r.packed.offsets[3] as u64 * 4,
            bytemuck::cast_slice(&vec![0u32; n + 1]),
        );
        for kind in [0u32, 1] {
            for distance in [0.001f32, 0.006] {
                let points = if kind == 0 {
                    [
                        [0.03f32, 0.05, distance],
                        [0., 0., 0.],
                        [0.1, 0., 0.],
                        [0., 0.1, 0.],
                    ]
                } else {
                    [
                        [-0.05, 0., distance],
                        [0.05, 0., distance],
                        [0., -0.05, 0.],
                        [0., 0.05, 0.],
                    ]
                };
                let weights = if kind == 0 {
                    [1., -0.2, -0.3, -0.5]
                } else {
                    [0.5, 0.5, -0.5, -0.5]
                };
                let mut positions = vec![[0f32; 4]; n];
                for (dst, p) in positions.iter_mut().zip(points) {
                    dst[..3].copy_from_slice(&p);
                }
                let direction: Vec<_> = (0..n)
                    .map(|i| glam::Vec3::new(i as f32 * 0.1, 0.2, (i as f32 * 0.7).cos()))
                    .collect();
                let particles: Vec<_> = direction
                    .iter()
                    .map(|p| {
                        let mut cg = [[0.; 4]; 7];
                        cg[2] = p.extend(0.).to_array();
                        Particle {
                            initial: [0.; 4],
                            velocity_mass: [0.; 4],
                            target: [0., 0., 0., 1.5],
                            delta: [0.; 4],
                            energy: [0.; 4],
                            cg,
                        }
                    })
                    .collect();
                gpu.queue
                    .write_buffer(&r.buffers[0], 0, bytemuck::cast_slice(&positions));
                gpu.queue
                    .write_buffer(&r.buffers[1], 0, bytemuck::cast_slice(&particles));
                let rho = 170000f32;
                let mut pair = vec![0u32; 24];
                pair[..5].copy_from_slice(&[0, 1, 2, 3, kind]);
                pair[5] = 0.001f32.to_bits();
                pair[7] = rho.to_bits();
                // Poison the cache: this must be rebuilt after AL variables/geometry.
                pair[12..].fill(f32::NAN.to_bits());
                gpu.queue
                    .write_buffer(&r.buffers[4], 0, bytemuck::cast_slice(&[1u32]));
                gpu.queue
                    .write_buffer(&r.buffers[4], 16, bytemuck::cast_slice(&pair));
                let mut config = vec![0u32; 40];
                config[0] = n as u32;
                config[1] = n as u32;
                config[6] = 1024;
                config[12..16].copy_from_slice(&r.packed.offsets);
                config[22] = 0.003f32.to_bits();
                config[23] = 100000f32.to_bits();
                config[27] = 1e-8f32.to_bits();
                config[32] = u32::from(method == ContactMethod::BarrierAl);
                config[36] = 8;
                gpu.queue
                    .write_buffer(&r.buffers[8], 0, bytemuck::cast_slice(&config));
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_compute_pass(&Default::default());
                    pass.set_bind_group(0, &r.bind, &[]);
                    for (entry, count) in [
                        (5, n.div_ceil(64)),
                        (6, 1),
                        (31, 1),
                        (15, n.div_ceil(64)),
                        (5, n.div_ceil(64)),
                        (32, 1),
                        (26, n.div_ceil(64)),
                    ] {
                        pass.set_pipeline(&gpu.pipelines[entry]);
                        pass.dispatch_workgroups(count as u32, 1, 1);
                    }
                }
                gpu.queue.submit([encoder.finish()]);
                let data = read_test_buffer(
                    &gpu,
                    &r.buffers[1],
                    (n * std::mem::size_of::<Particle>()) as u64,
                );
                let actual: &[Particle] = bytemuck::cast_slice(&data);
                let curvature = if method == ContactMethod::Ipc {
                    crate::ipc::barrier(distance as f64, 0.003f32 as f64, 100000.)[2]
                } else {
                    rho as f64
                };
                let dotp: f64 = (0..4).map(|i| weights[i] * direction[i].z as f64).sum();
                let barrier = if method == ContactMethod::Ipc {
                    crate::ipc::barrier(distance as f64, 0.003f32 as f64, 100000.)
                } else {
                    crate::barrier_al::Contact {
                        slack: 0.001f32 as f64,
                        dual: 0.,
                        rho: rho as f64,
                    }
                    .primal(distance as f64, 0.003f32 as f64, 100000.)
                };
                for v in 0..n {
                    let x =
                        glam::Vec3::from_array(positions[v][..3].try_into().unwrap()).as_dvec3();
                    let mut gradient = 1.5 * x;
                    let mut energy = 0.75 * x.length_squared();
                    if v < 4 {
                        gradient.z += barrier[1] * weights[v];
                        energy += barrier[0] / 4.;
                    }
                    let got_gradient =
                        glam::Vec3::from_array(actual[v].delta[..3].try_into().unwrap()).as_dvec3();
                    assert!(
                        got_gradient.distance(gradient) < 0.002 + gradient.length() * 3e-5,
                        "cached {method:?} contact gradient differs from f64"
                    );
                    assert!(
                        (actual[v].delta[3] as f64 - energy).abs() < 1e-6 + energy.abs() * 1e-4,
                        "scalar contact energy differs from f64"
                    );
                    let mut expected = 1.5 * direction[v].as_dvec3();
                    if v < 4 {
                        expected.z += curvature * weights[v] * dotp;
                    }
                    let got =
                        glam::Vec3::from_array(actual[v].cg[3][..3].try_into().unwrap()).as_dvec3();
                    assert!(
                        got.distance(expected) < 0.05 + expected.length() * 3e-5,
                        "{method:?} kind={kind} v={v}: {got:?} != {expected:?}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires explicit coordination and a hardware GPU"]
fn separating_axis_filter_retains_close_vf_ee_and_nearly_parallel_pairs() {
    let (device, queue) = request_device().unwrap();
    let gpu = GpuAvbd::new(device, queue);
    let mut template = GarmentTemplate::two_panel_demo();
    for p in &mut template.panels {
        p.pattern.nx = 12;
        p.pattern.ny = 12;
    }
    let garment = Arc::new(template.compile().unwrap());
    let state = garment.initial_state.clone();
    let solver = VbdSolver::new(garment, None, &state).unwrap();
    let r = gpu.allocate(&solver, &state, None, 1024).unwrap();
    let source=[include_str!("main.wgsl"),include_str!("jet.wgsl"),include_str!("contact.wgsl"),
        include_str!("solve.wgsl"),include_str!("al.wgsl"),include_str!("coupled.wgsl"),
        "@compute @workgroup_size(64) fn probe_filter(@builtin(global_invocation_id) id:vec3u) { if id.x<64u {contacts.pairs[id.x].normal_curvature.w=select(0.0,1.0,separated_beyond_envelope(contacts.pairs[id.x]));}}"].join("\n");
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pair-filter oracle"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&gpu.layout)],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&layout),
            module: &module,
            entry_point: Some("probe_filter"),
            compilation_options: Default::default(),
            cache: None,
        });
    for translation in [0f32, 10., 100.] {
        let mut positions = vec![[0f32; 4]; state.x.len()];
        let mut pairs = Vec::new();
        let mut close = Vec::new();
        for i in 0..64 {
            let rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                i as f32 * 0.27,
                i as f32 * 0.53,
                i as f32 * 0.17,
            );
            let u = rotation * glam::Vec3::X;
            let v = rotation * glam::Vec3::Y;
            let normal = rotation * glam::Vec3::Z;
            let case = i % 4;
            let kind = u32::from(i >= 32);
            let p = if kind == 0 {
                let point = match case {
                    0 => u * 0.03 + v * 0.05 + normal * 0.002,
                    1 => normal * 0.002,
                    2 => -u * 0.1 - v * 0.1 + normal * 0.001,
                    _ => u * 0.03 + v * 0.05 + normal * 0.02,
                };
                [point, glam::Vec3::ZERO, u * 0.1, v * 0.1]
            } else {
                let direction = (u + v * if i % 2 == 0 { 1e-5 } else { 0.8 }).normalize();
                let d = if case == 2 {
                    0.02
                } else if case == 3 {
                    0.
                } else {
                    0.002
                };
                [
                    -u * 0.05,
                    u * 0.05,
                    -direction * 0.05 + normal * d,
                    direction * 0.05 + normal * d,
                ]
            };
            close.push(case < 2 || (kind == 1 && case == 3));
            for j in 0..4 {
                positions[4 * i + j] = (p[j] + glam::Vec3::splat(translation))
                    .extend(0.)
                    .to_array();
            }
            let mut pair = [0u32; 24];
            pair[..5].copy_from_slice(&[
                (4 * i) as u32,
                (4 * i + 1) as u32,
                (4 * i + 2) as u32,
                (4 * i + 3) as u32,
                kind,
            ]);
            pairs.extend(pair);
        }
        gpu.queue
            .write_buffer(&r.buffers[0], 0, bytemuck::cast_slice(&positions));
        gpu.queue
            .write_buffer(&r.buffers[4], 16, bytemuck::cast_slice(&pairs));
        let mut config = vec![0u32; 40];
        config[22] = 0.003f32.to_bits();
        config[27] = (64. * f32::EPSILON * translation.max(1.)).to_bits();
        config[35] = 0.25f32.to_bits();
        gpu.queue
            .write_buffer(&r.buffers[8], 0, bytemuck::cast_slice(&config));
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_bind_group(0, &r.bind, &[]);
            pass.set_pipeline(&pipeline);
            pass.dispatch_workgroups(1, 1, 1);
        }
        gpu.queue.submit([encoder.finish()]);
        let result = read_test_buffer(&gpu, &r.buffers[4], 16 + 64 * 96);
        let mut rejected = 0;
        for (i, pair) in result[4..].as_chunks::<24>().0.iter().enumerate() {
            let culled = f32::from_bits(pair[15]) > 0.;
            if close[i] {
                assert!(
                    !culled,
                    "lost known close witness {i} at translation {translation}"
                );
            }
            rejected += usize::from(culled);
        }
        assert!(
            rejected >= 12,
            "filter must reject separated VF/EE pairs: {rejected}"
        );
    }
}

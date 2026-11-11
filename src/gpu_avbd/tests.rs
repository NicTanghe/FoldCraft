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
    let history = &data[4 + r.capacity * 12..4 + (r.capacity + count) * 12];
    let old: Vec<Vec<u32>> = history
        .as_chunks::<12>()
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
    for p in actual[4..4 + count * 12].as_chunks::<12>().0 {
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

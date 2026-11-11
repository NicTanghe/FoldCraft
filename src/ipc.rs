//! Independent squared-distance IPC barrier oracle. SI units; no IPC Toolkit code.
//! This scalar potential alone is not a collision or convergence guarantee.

/// Energy (J), derivative (N), and second derivative (N/m) with respect to
/// distance in metres. `stiffness` is N/m; the logarithm is dimensionless.
/// B = -k r²/8 (d²/r² - 1)² log(d²/r²), for 0 < d < r.
pub fn barrier(distance: f64, radius: f64, stiffness: f64) -> [f64; 3] {
    if distance >= radius {
        return [0.; 3];
    }
    if distance <= 0. {
        return [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY];
    }
    let s = (distance / radius).powi(2);
    let u = s - 1.;
    let log = s.ln();
    let scale = stiffness * radius * radius / 8.;
    let f = -u * u * log;
    let first = -(2. * u * log + u * u / s);
    let second = -(2. * log + 4. * u / s - u * u / (s * s));
    [
        scale * f,
        scale * first * 2. * distance / radius.powi(2),
        scale * (second * 4. * distance.powi(2) / radius.powi(4) + first * 2. / radius.powi(2)),
    ]
}

/// Conservative fraction of a simultaneous linear primitive update. Distance
/// is 1-Lipschitz in each primitive's maximum vertex displacement. Subtract a
/// numerical clearance before bounding the *entire path*, not just its end.
/// The caller must cover all pairs within 2 * max_displacement + clearance.
pub fn step_fraction(distance: f64, a_motion: f64, b_motion: f64, clearance: f64) -> f64 {
    if distance <= clearance {
        return 0.;
    }
    (0.8 * (distance - clearance) / (a_motion + b_motion).max(f64::MIN_POSITIVE)).clamp(0., 1.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derivatives_and_c2_activation() {
        let r: f64 = 0.003;
        let k = 100_000.;
        for ratio in [0.01, 0.1, 0.4, 0.8, 0.99] {
            let d = ratio * r;
            let h = d.min(r - d) * 1e-5;
            let [e, g, hh] = barrier(d, r, k);
            let plus = barrier(d + h, r, k);
            let minus = barrier(d - h, r, k);
            assert!(e > 0. && g < 0. && hh > 0.);
            assert!(((plus[0] - minus[0]) / (2. * h) - g).abs() < g.abs() * 1e-7);
            assert!(((plus[1] - minus[1]) / (2. * h) - hh).abs() < hh * 1e-7);
        }
        let near = barrier(r * (1. - 1e-8), r, k);
        assert!(near[0] < 1e-20 && near[1].abs() < 1e-10 && near[2] < 0.01);
        assert_eq!(barrier(r, r, k), [0.; 3]);
        assert!(barrier(0., r, k)[0].is_infinite());
    }
    #[test]
    fn simultaneous_motion_cannot_consume_clearance() {
        // Both primitives move, including an edge crossing with separated ends.
        for d in [1e-5, 0.001, 1.] {
            for a in [0., 0.001, 10.] {
                for b in [0., 0.002, 20.] {
                    let epsilon = 1e-6;
                    let alpha = step_fraction(d, a, b, epsilon);
                    assert!(d - alpha * (a + b) > epsilon);
                }
            }
        }
        assert_eq!(step_fraction(1e-6, 1., 1., 1e-6), 0.);
    }
}

//! Independent positive-distance splitting oracle for experimental barrier AL.
//! This is not the published BAL contact formulation. All values use SI units.
use crate::ipc::barrier;

/// A frozen contact state. The multiplier is unscaled (N), not an impulse.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub slack: f64,
    pub dual: f64,
    pub rho: f64,
}

/// Curvature-balanced augmentation, bounded relative to the effective mechanical
/// contact curvature. This is a heuristic for coupled cloth, not a convergence bound.
pub fn augmentation(mechanical: f64, barrier_curvature: f64, stiffness: f64) -> f64 {
    (mechanical * barrier_curvature.max(mechanical * 0.01))
        .sqrt()
        .clamp(0.1 * mechanical, 100. * mechanical)
        .max(stiffness)
}

impl Contact {
    pub fn new(distance: f64, radius: f64, stiffness: f64, mechanical: f64) -> Self {
        let b = barrier(distance, radius, stiffness);
        Self {
            slack: distance,
            dual: b[1],
            rho: augmentation(mechanical, b[2], stiffness),
        }
    }

    /// Frozen shifted augmented energy, its distance derivative and curvature.
    /// The shift dual²/(2 rho) is constant only within a primal solve.
    pub fn primal(self, distance: f64, radius: f64, stiffness: f64) -> [f64; 3] {
        let q = distance - self.slack + self.dual / self.rho;
        [
            barrier(self.slack, radius, stiffness)[0] + 0.5 * self.rho * q * q,
            self.rho * q,
            self.rho,
        ]
    }

    /// Exact bracketed scalar slack minimization followed by the unscaled dual
    /// update. No mesh mutation; invalid input is rejected transactionally.
    pub fn update(
        &mut self,
        distance: f64,
        radius: f64,
        stiffness: f64,
    ) -> Result<(), &'static str> {
        if [distance, radius, stiffness, self.slack, self.rho]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
            || !self.dual.is_finite()
        {
            return Err("invalid barrier AL scalar state");
        }
        let mut lo = radius * 1e-14;
        let mut hi = 2. * radius.max(distance + self.dual.abs() / self.rho);
        let residual = |z| barrier(z, radius, stiffness)[1] - self.dual + self.rho * (z - distance);
        if !hi.is_finite() || residual(lo) >= 0. || residual(hi) < 0. {
            return Err("barrier AL slack root is not bracketed");
        }
        for _ in 0..80 {
            let z = 0.5 * (lo + hi);
            if residual(z) > 0. {
                hi = z;
            } else {
                lo = z;
            }
        }
        let slack = 0.5 * (lo + hi);
        let dual = self.dual + self.rho * (distance - slack);
        if !slack.is_finite() || !dual.is_finite() {
            return Err("nonfinite barrier AL update");
        }
        self.slack = slack;
        self.dual = dual;
        Ok(())
    }
}

/// Experimental diagnostics; these are not an equilibrium certificate.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Diagnostics {
    pub primal_m: f64,
    pub slack_stationarity_n: f64,
    pub dual_change_n: f64,
    pub original_force_max_n: f64,
    pub armijo_trials: u32,
    pub matched_pairs: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_derivatives_and_fixed_point_match_original_barrier() {
        for d in [0.00003, 0.0003, 0.002, 0.004] {
            let c = Contact::new(d, 0.003, 1e5, 2e4);
            let h = d * 1e-5;
            let value = c.primal(d, 0.003, 1e5);
            let plus = c.primal(d + h, 0.003, 1e5);
            let minus = c.primal(d - h, 0.003, 1e5);
            assert!(
                ((plus[0] - minus[0]) / (2. * h) - value[1]).abs() < 1e-5 * value[1].abs().max(1.)
            );
            assert!(((plus[1] - minus[1]) / (2. * h) - value[2]).abs() < 1e-6 * value[2]);
            assert!((value[1] - barrier(d, 0.003, 1e5)[1]).abs() < 1e-9);
            let mut updated = c;
            updated.update(d, 0.003, 1e5).unwrap();
            assert!((updated.slack - d).abs() < 1e-12);
            assert!((updated.dual - c.dual).abs() < 1e-8);
        }
    }

    #[test]
    fn balanced_scalar_contact_converges_and_releases() {
        for mechanical in [0.1, 1., 10., 100., 1000.] {
            let mut distance = 0.9;
            let mut c = Contact::new(distance, 1., 1., mechanical);
            for target in [-0.5, 2.] {
                let mut converged = false;
                for _ in 0..200 {
                    c.rho = augmentation(mechanical, barrier(distance, 1., 1.)[2], 1.);
                    let trial =
                        (mechanical * target + c.rho * c.slack - c.dual) / (mechanical + c.rho);
                    let delta = trial - distance;
                    let alpha = if delta < 0. {
                        (0.8 * (distance - 1e-12) / (-delta)).min(1.)
                    } else {
                        1.
                    };
                    distance += alpha * delta;
                    c.update(distance, 1., 1.).unwrap();
                    let force = mechanical * (distance - target) + barrier(distance, 1., 1.)[1];
                    if (distance - c.slack).abs() < 1e-8 && force.abs() < 1e-7 {
                        converged = true;
                        break;
                    }
                }
                assert!(converged, "mechanical={mechanical}, target={target}");
                assert!(distance > 0.);
                if target > 1. {
                    assert!((distance - target).abs() < 1e-6 && c.dual.abs() < 1e-7);
                }
            }
        }
    }

    #[test]
    fn invalid_slack_update_does_not_mutate_state() {
        let mut c = Contact::new(0.001, 0.003, 1e5, 100.);
        let old = c;
        assert!(c.update(f64::NAN, 0.003, 1e5).is_err());
        assert_eq!(c.slack, old.slack);
        assert_eq!(c.dual, old.dual);
    }
}

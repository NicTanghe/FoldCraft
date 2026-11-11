# AVBD + barrier-augmented Lagrangian contact: implementation design

11 November 2026 — FC-0036. **Design, not an implemented or validated backend.**
The native solver dropdown is implemented separately. The WGSL AVBD/IPC default
and existing CPU controls remain available; this proposal must not appear as a
working solver choice until its numerical and integration gates pass.

## Decision and scope

Implement an experimental GPU-resident contact splitting method around the
existing finite-material AVBD/Jacobi primal solver. Keep the same rest mesh,
mass, anisotropic membrane, bending, finite sewing, pins, attachments, timestep
and damping. Contact must participate in the implicit objective; no positional
repair after the solve.

The first experiment should preserve the existing scalar IPC-style barrier at
convergence. It will introduce positive distance slacks and contact multipliers
to separate steep barrier curvature from the local position solves. This is an
independent proposed hybrid, not a reproduction of either complete paper.
The principal hypothesis is that cheaper, better-conditioned local position
updates outweigh slack/dual updates and the extra iterations needed to propagate
coupled error. That hypothesis may fail.

The requested last column is in [issue 242](https://github.com/erythrofr/ClothCraft/issues/242).
The issue explicitly describes a proposed combination, not a published solver.

## Evidence and important distinction

The [BAL paper, arXiv v1](https://arxiv.org/html/2407.00046v1) was consulted,
particularly §3, Algorithm 1, and §§4–5. Its formulation retains a base barrier
and adds a multiplier term with a slack-dependent barrier support on an adaptive
augmentation set. Its primal solver is Newton-PCG, not AVBD. Its conditioning,
convergence and speed results therefore do not establish this hybrid's behavior.

The [AVBD project](https://graphics.cs.utah.edu/research/projects/avbd/) and the
repository's [finite AVBD implementation notes](wgsl-avbd-ipc.md) are the other
starting points. The current finite-material ramp is not the complete hard
constraint AVBD algorithm. No external implementation has been copied.

Two approaches should remain distinguishable:

| Approach | Benefit | Cost / unresolved question |
|---|---|---|
| Adapt the paper's adaptive augmentation and moving barrier support | Closest to the cited BAL contact formulation | Requires a new scalar oracle, careful units/dual convention, changing support coverage, and a new convergence argument for AVBD replacing PCG |
| Split the current barrier through positive distance slacks | Explicit fixed-point equivalence to today's contact energy; permits a controlled matched-objective experiment | Not the paper's BAL formulation; temporary forces can differ substantially until primal/slack agreement |

**Proposed first implementation: the second approach**, labelled
“WGSL AVBD / barrier AL (experimental)”. Do not label it as the authors' BAL
solver. Keep a literal paper adaptation as a separate follow-up rather than
quietly changing the first experiment's equations.

## Independent formulation in physical units

Let h be the timestep, D mass-proportional damping, and M the lumped mass matrix.
Use the existing inertial target y and objective

    F(x) = 1/2 (x-y)^T [M (1+h D)/h²] (x-y) + U(x).

U includes membrane, bending, sewing and grabs. Pin degrees of freedom are
eliminated. Rest geometry comes only from the material coordinates.

For primitive pair i, d_i(x) is its unsigned finite-primitive separation.
For the analytic sphere, use the triangle/sphere gap, including the established
thickness offset. Let r be activation distance and k_i contact stiffness.
Keep the current contact law:

    q = d²/r²
    B_i(d) = -(k_i r²/8) (q-1)² log(q),  0 < d < r
    B_i(d) = 0,                         d >= r.

B has units J, B' has units N, and B'' has units N/m. Existing issues with
primitive-pair duplication, feature transitions and unmollified EE contacts
are inherited; this work does not fix those issues by changing the optimizer.

Introduce z_i > 0 and the equality d_i(x)-z_i = 0:

    minimize F(x) + sum_i B_i(z_i)
    subject to d_i(x)-z_i = 0.

With multiplier lambda_i in N and augmentation rho_i > 0 in N/m:

    L(x,z,lambda) =
        F(x) + sum_i [ B_i(z_i)
                    + lambda_i (d_i(x)-z_i)
                    + rho_i/2 (d_i(x)-z_i)² ].

At a stationary feasible point, z_i=d_i(x) and lambda_i=B_i'(z_i).
The position gradient is then exactly the gradient of F + sum_i B_i(d_i).
This gives an independently testable fixed-point target, not a proof that a
finite number of alternating steps converges for nonsmooth cloth contact.

### Position update

Hold z, lambda and rho fixed during a batch of primal iterations. Define

    a_i = lambda_i + rho_i (d_i-z_i)
    grad_x L_i = a_i grad_x d_i
    Hessian_x L_i =
        rho_i grad_x d_i grad_x d_i^T + a_i Hessian_x d_i.

Use exact gradients and a documented positive local curvature approximation
for the existing per-vertex 3×3 solve. Initially retain the PSD outer-product
term and the existing inertial/material curvature. Missing distance curvature
makes this a search-direction approximation, not an exact Newton method.

Vertices gather floating contributions using the existing integer-atomic
incidence lists. Do not use unbounded fixed-point force atomics. Simultaneous
Jacobi updates require a global descent test on the **frozen augmented
objective**, not independently accepted per-vertex moves.

For numerical convenience evaluate each contact contribution as

    B_i(z_i) + rho_i/2 [d_i-z_i + lambda_i/rho_i]².

It differs from L by a constant for the frozen primal batch and gives the same
gradient and Armijo decision. That constant changes after dual updates: never
compare these energies across dual iterations as physical dissipation evidence.

### Slack and dual update

At fixed x, independently minimize each one-dimensional slack problem:

    B_i'(z_i) - lambda_i + rho_i (z_i-d_i) = 0,  z_i > 0.

First implement a bracketed f64 scalar oracle. When B'' >= 0, its derivative is
B''+rho > 0, so a bracketed root is unique. The GPU version should use safeguarded
Newton with a bisection fallback and record failure/nonfinite results.
Do not clamp a failed root to zero or accept an unresolved singularity.

After the slack solve:

    lambda_i <- lambda_i + rho_i (d_i-z_i).

Check the sign: at slack stationarity the updated lambda equals B'(z), which is
nonpositive for the repulsive barrier. With this convention the contact force
is -lambda grad(d). Do not blindly apply a nonnegative multiplier clamp copied
from an inequality formulation with the opposite sign.

Initial state for a new pair: z=d, lambda=B'(d), rho=rho_initial.
Cold-start all duals at each physical timestep for the first implementation.
Preserve duals across nonlinear iterations within that timestep. Warm starts
across physical timesteps require a separate identity/energy study.

Start with fixed rho = k_i times a dimensionless, reported factor. Sweep that
factor in tests instead of silently treating it as a material parameter.
Adaptive rho is a later experiment: retain the unscaled multiplier when rho
changes, and measure both residuals before claiming an improvement.

### Residuals and finite-budget behavior

Record at least:

- primal distance residual: max_i |d_i-z_i|, metres;
- slack stationarity residual: max_i |B'(z_i)-lambda_i+rho_i(z_i-d_i)|, N,
  evaluated before the dual update;
- dual-change proxy: max_i rho_i |z_i(new)-z_i(old)|, N;
- free-DOF gradient norm of the original F + B(d), N, at final physical stiffness;
- minimum independently audited surface separation and accepted line fraction;
- primal sweeps, slack iterations, Armijo trials and candidate counts.

A small velocity is not convergence. A small augmented gradient alone is not
original-objective force balance. Report budget exhaustion independently of
numerical failure. Initially expose this as an experiment with explicit residual
and budget reporting; do not replace the default solver on the basis of a small
smoke case.

## Collision safety and candidate lifetime

Slack positivity does not prevent the actual mesh from intersecting. Retain the
existing continuous Lipschitz path bound on the **actual positions**:

    alpha <= 0.8 (d-clearance) / (max_motion_A + max_motion_B).

Check every eligible VF, reciprocal VF and EE pair, including self-contact,
with the existing local sewing exclusions. Recompute distances and bounds for
every simultaneous position update. No repair of initially intersecting input.

Keep the proposal limit L=r/4 initially. A detector reused for m primal updates
must cover at least r + 2mL plus numerical padding. For m=1, retain the existing
r+2L coverage. The simplest first version refreshes every primal update; any
multi-update reuse needs an explicit accumulated-motion proof and comparison
against exhaustive candidates.

There are two different sets:

1. Current safety candidates, covering every possible collision during the
   update, including pairs not currently producing a barrier force.
2. Persistent augmented pairs, retaining their z/lambda/rho until a controlled
   release or reset.

Their union must be evaluated. A pair leaving the current search radius must
not silently lose its nonzero multiplier. Retain it until its barrier force,
slack mismatch and multiplier are all negligible under declared tolerances.
Non-augmented newly discovered pairs use the base barrier until initialized,
so entering contact never has a missing-force interval.

The persistent set can grow. Capacity overflow must grow and retry the entire
physical step from its accepted checkpoint, or fail transactionally at device
limits. Never truncate contacts or reuse a slot with an unrelated multiplier.

## GPU state and dispatch plan

Keep the current eight-storage-buffer design if practical by using explicit
regions in packed buffers. Record offsets/strides in one Rust/WGSL layout contract
and validate shader parsing and buffer sizes without a GPU.

Persistent records need a generation, canonical primitive key, kind, z, lambda,
rho and lifecycle flags. A slot number from atomic insertion is not an identity.

For the first correct implementation, use a GPU key table with collision-resolved
lookup and a bounded load factor, or previous/current contact lists indexed by
incident moving vertex. Compare full keys, including primitive kind. Keep
read/write generations separate; insert, initialize, publish and consume in
separate dispatches. Avoid spin locks that assume workgroups make simultaneous
progress. A hash collision must never transfer state to another pair.

VF keys distinguish cloth-to-avatar from avatar-to-cloth and self-contact.
EE keys use canonical edge identities, not current closest-feature weights.
Sphere records use stable triangle identity within the compiled generation.
Remeshing, material edits, collider replacement and UI solver switching reset
all these records.

Proposed physical step:

1. Validate changed controls; snapshot accepted state; reset per-step dual state.
2. Build inertial targets and initialize finite material continuation.
3. Refit/search, merge persistent augmented pairs, build incidence.
4. Initialize new pair slacks and multipliers; compute residuals.
5. Assemble per-vertex primal direction using frozen dual/slack state.
6. Bound the simultaneous path and perform global Armijo backtracking.
7. Commit an accepted primal update, or retain the prior iterate on exhaustion.
8. Solve slacks, update duals and lifecycle flags; recompute residuals.
9. Repeat to tolerance or explicit budget. Reach full material stiffness before
   testing final force convergence.
10. Audit status, integrate velocity, and publish one accepted-state snapshot.

All iteration decisions remain on GPU. One final host readback serves existing
UI/export consumers; retry/error readbacks are counted separately.
GPU failure leaves CPU x/v/time unchanged and invalidates unaccepted residency.

## Native selector contract

Click the solver badge in the top bar to open the dropdown. Fitting offers
WGSL AVBD/IPC and CPU VBD/OGC; Project v1 offers WGSL AVBD/IPC and CPU reference
Newton. These CPU contact laws are not equivalent to the IPC objective.

Switching pauses playback at the last displayed accepted x/v/time. Increment
the worker generation, clear obsolete metrics and solver caches, and discard
in-flight results from the prior generation. Do not alter rest geometry,
document/history or serialized file versions. Press Play or Step to advance
the chosen solver. CPU-launched windows may initialize GPU computation lazily
on the first GPU step. Adapter failure is visible; no silent CPU fallback.

The future experimental choice follows the same policy. It is absent until
implemented; a label that runs the IPC backend would be misleading.

## Validation and optimization sequence

1. **CPU math gate:** finite differences of primal energy/gradient, scalar slack
   roots across near-contact and inactive ranges, sign/units, stationary
   equivalence to B(d), convergence on a one-dimensional inertial contact
   problem, separation/release with no persistent attraction.
2. **GPU parity gate:** f32-rounded oracle comparisons for energy, gradients,
   slack roots and dual updates, with bounded numerical tolerances.
3. **Lifecycle gate:** shuffled contact insertion, key collisions, pair creation
   and deletion, overflow/retry, reset/remesh, GPU→CPU→GPU switches, stale worker
   completion and errors. Default tests remain CPU-only.
4. **Physics gate:** freefall, pins, objectivity, reciprocal VF, EE crossings,
   almost-parallel edges, high-speed sphere impacts, layered cloth, translation,
   matched seam closure and pull/hold/release. Independent final separation
   audit is required; inspect residuals, energy and damping as well as pictures.
5. **Cost gate:** same mesh, material, dt, initial x/v/time and control trajectory.
   Compare equal error/quality targets, not equal iteration counts. Report full
   step median/p95, simulation time per wall second, residuals, minimum gap,
   memory, retries, and separately startup/shader compilation/readback.
   Timestamp GPU phases only when supported; window FPS is a separate metric.

Prioritized optimization hypotheses, after correctness:

- Evaluate scalar energy only during Armijo trials; avoid unnecessary AD
  gradients/Hessians and repeated per-vertex evaluation of shared factors.
- Cache frozen contact geometry/curvature only for a justified lifetime; trial
  energy and safety bounds still use current actual distances.
- Use GPU indirect dispatch/termination flags to avoid submitting all 16
  backtracking trials after an early acceptance, without a CPU poll.
- Parallelize global reductions in two stages when one workgroup becomes a
  measurable bottleneck.
- Reuse capacity and unchanged topology/material buffers; count actual transfers.
- Consider coupled seam blocks or a matrix-free PCG correction only if residual
  traces show long-range error dominates. This changes the primal strategy and
  needs separate tests.

Every retained optimization needs a before/after ablation on identical inputs.
If it only speeds a kernel, label that result as such. No speedup, stability,
full BAL equivalence or dense-garment qualification has been measured for this
proposed solver.

## Implementation checkpoints

A. Native selector and this design document.
B. CPU scalar/contact-splitting oracle and residual definitions.
C. GPU persistent contact state with exhaustive identity/overflow checks.
D. Opt-in hybrid kernels and native/CLI integration.
E. Small authorized GPU validation and matched full-step cost measurements.
F. Optimization with retained baselines and documented failed experiments.

Checkpoint A is the current deliverable. B–F remain implementation work.

## First design experiment: augmentation scale can make or break the solve

A CPU-only scalar prototype is saved in
[scripts/research-barrier-al.py](../scripts/research-barrier-al.py), with
[raw results](avbd-barrier-al-scalar-study.json). Reproduce with:

    python3 scripts/research-barrier-al.py

This is a dimensionless one-contact convex model, r=k=1, target y=-0.5,
initial d=0.9, exact scalar primal minimization, a conservative positive-distance
path fraction, bracketed slack solve, and the dual update above. A separate
bracketed root of the original objective supplies the reference solution.
It is not a cloth implementation, GPU check or timing benchmark.

Observed iterations to primal mismatch < 1e-8 and original force residual < 1e-7:

| Inertial curvature a | rho=0.1 | rho=1 | rho=10 | rho=100 |
|---|---:|---:|---:|---:|
| 0.1 | 27 | 23 | 131 | 1,248 |
| 1 | 145 | 27 | 57 | 487 |
| 10 | 2,124 | 222 | 33 | 34 |
| 100 | **failed at 10,000** | 6,988 | 711 | 82 |

The failed case approaches the numerical clearance while keeping a very large
original force residual. It stays separated, which demonstrates why separation
alone cannot validate this solver. A fixed rho tied only to barrier stiffness
is not an adequate default across mass and timestep scales.

For locally quadratic scalar objectives with positive curvatures a and b,
exact alternating primal/slack solves have nonzero linearized contraction

    t(rho) = (rho² + a b) / [(a+rho)(b+rho)].

This follows by substituting the scalar primal and slack solutions into the
unscaled dual update. Its minimum occurs at rho=sqrt(a b). It is a local
diagnostic for exact scalar solves, not a convergence bound for cloth Jacobi
updates or changing closest features.

**Design revision:** test rho against both local mechanical curvature and
barrier curvature before porting the method. A candidate estimate for a contact
with gradient g and positive mechanical block diagonal H is
a_eff = 1 / (g^T H^-1 g), in N/m. Compare rho=k, rho=a_eff and a bounded
sqrt(a_eff B''(d)) policy using identical initial states. Treat small/inactive
barrier curvature and fixed vertices explicitly. Changing rho keeps lambda
unscaled; document and test any bounds and update cadence. Do not add an
adaptive rule to the GPU merely because the scalar formula looks promising.

Other decisions required before the kernel implementation:

- Augmented pairs replace their direct B(d) contribution; they do not add a
  second physical contact barrier. Non-augmented pairs retain direct B(d).
- An AL primal force can temporarily attract during separation even when the
  updated multiplier has the repulsive sign. Test release and tangential motion
  and report finite-budget artifacts. Do not hide them with position projection.
- For k=0, skip augmentation rather than dividing by rho=0.
- Distinguish the mathematical slack domain z>0 from the actual mesh's numerical
  clearance. Bracketing failures require an explicit numerical status.
- The initial scope is static triangle avatars, cloth self-contact and the
  existing analytic Project sphere. Animated collision geometry, friction,
  inversion protection and full published IPC deduplication are separate work.
- No finite-budget hybrid should be promoted until original-force residual,
  seam error, release behavior and full-step cost pass together.

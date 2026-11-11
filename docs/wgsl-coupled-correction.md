# Coupled GPU correction for the default T-shirt protrusion

FC-0043, 11 November 2026. Both resident WGSL backends now use an inexact coupled
Gauss-Newton/PCG search instead of the former single vertex-block Jacobi search.
The CPU solver, CPU OGC model, material energy code and physical settings are
unchanged. This is an independent implementation; no external solver code was
copied and no complete published AVBD/IPC/BAL equivalence is claimed.

## Diagnosis and rejected changes

The initial global contact-bound correction did not resolve the reported shape.
The full reset default garment (2454 vertices / 4570 triangles) reproduced the
raised underarm/sleeve-root shape in both WGSL backends. After 200 steps at
dt=1/60 with 16 sweeps:

| Backend before this correction | Final maximum extension | Maximum seam gap |
|---|---:|---:|
| CPU VBD/OGC control | 21.174% | 2.041 mm |
| WGSL IPC Jacobi | 37.258% | 4.147 mm |
| WGSL Barrier AL Jacobi | 36.747% | 4.038 mm |

Increasing the proposal envelope from r/4 to r, with detection enlarged to r+2L,
only improved IPC extension to 35.442% and roughly doubled complete run time.
It was rejected as the fix; the default envelope remains r/4. Ten additional
IPC steps at 160 sweeps reduced extension to 31.213%, still substantially above
the control. Restarting the bad state under CPU for 30 steps also did not
immediately undo its assembly history. These experiments motivated a coupled
search rather than a material, damping, mesh or CPU change.

The per-vertex AVBD row-norm curvature also resists rigid translation of a
stressed patch when treated as a global approximation. The coupled search uses
the positive Gauss-Newton operator below, with cross-vertex contributions.

## Operator and GPU passes

The physical gradient is unchanged. For material/sewing/attachment factors
U=sum(k C²/2), use

    H = A + sum k grad(C) grad(C)^T
          + sum max(B''(d),0) grad(d) grad(d)^T.

A is the existing implicit mass/drag diagonal. Contact uses the frozen AL
primal curvature for the AL variant; sphere terms use the existing triangle
distance and area-integrated stiffness. Pins eliminate degrees of freedom.
This is a positive search curvature, not the complete energy Hessian:
nonlinear second-distance and C Hessian terms are omitted. Global Armijo still
checks the actual current objective, not the quadratic approximation.

Each nonlinear sweep caches factor gradients at the frozen iterate, then
performs eight PCG iterations with vertex-block preconditioning. The block
preconditioner gets a numerical floor

    max(1e-5 trace(D), 1e-12 N/m) I.

Only the preconditioner is regularized: neither H nor physical energy/stiffness
is changed. This prevents f32 inversion failure for nearly singular blocks;
the initial unconditioned AL trial rejected a step transactionally and was
not retained.

Matrix-vector products gather all participating vertex search directions.
Reductions, alpha/beta updates, residuals and search directions remain on GPU.
There are no linear-iteration CPU readbacks or CPU solve fallbacks. The final
direction is bounded by the existing proposal envelope and per-vertex contact
balls, its Armijo slope is updated, and the complete simultaneous update passes
the existing continuous path and energy checks. AL slack/dual updates follow
acceptance as before. The final AL original-force audit rebuilds the factor
cache at the final geometry and full material stiffness.

The same eight storage bindings are retained. Particle scratch and factor
gradient storage grow; allocation limits still reject rather than truncate.
Cache contents are numerical GPU workspace, not serialized garment geometry.

## Matched default-garment result

After 200 steps from the same reset source:

| Final search | Final maximum extension | Maximum seam gap | Vertex RMS difference from CPU |
|---|---:|---:|---:|
| CPU control | 21.174% | 2.041 mm | — |
| Coupled WGSL IPC | 19.957% | 2.181 mm | 5.831 mm |
| Coupled WGSL Barrier AL | 21.307% | 2.250 mm | 4.365 mm |

Before correction IPC differed by 24.749 mm RMS and up to 77.5 mm at sewn
underarm points. At the previously worst underarm witness the final differences
are 4.422 mm (IPC) and 2.020 mm (AL). Identical-camera saved-state renders,
including a lower viewing angle, show the raised region removed in these runs.
Both final states passed the unchanged CPU intersection-start validator without
advancing them. No after-the-fact vertex projection was applied.

Raw run summaries are in protrusion-matched-runs.json. Full local checkpoints,
OBJ files and PNG comparisons are under output/protrusion-repro. These results
qualify this default static reproduction, not all garments or dynamic contact.

The correction costs more work: 16 nonlinear sweeps now include 8 linear
iterations each. Complete 200-step runs were about 102 s IPC and 97 s AL on this
machine, compared with about 48 s / 81 s for the old searches and 50 s for the
CPU control. Compilation is excluded; these are not portable performance
benchmarks or GPU timestamps. Quality, not a speedup, is the measured result.

## Reproduction and regression

    # Fresh default document; avoid replaying a cached distorted state.
    cargo run --release --locked -j 2 --bin cloth-fit -- --steps 0 --view

    # Headless ablations (not native UI options)
    cloth-fit --project RESET.json --coupled-iterations 0 --steps 200 --output OLD
    cloth-fit --project RESET.json --coupled-iterations 8 --steps 200 --output NEW
    cloth-fit --project RESET.json --proposal-radii 1 --coupled-iterations 0 --steps 200 --output ENVELOPE

The AL flag selects the same experiments with distance-slack contact. Outputs
record search revision, linear budget and proposal envelope. Existing backend
identifiers and document versions are preserved.

The long regression is a separate opt-in target:

    RAYON_NUM_THREADS=2 cargo test --release --locked -j 2 --test gpu_garment_quality -- --ignored --test-threads=1 --nocapture

It uses the public default asset/template, runs the unchanged CPU control and
both WGSL paths for 200 steps, independently audits separation every 50 GPU
steps, and bounds final seam error, extension and displacement relative to the
CPU control. It is not included in the small GPU test command. Ordinary tests
never initialize a GPU.

Additional small GPU regressions compare the coupled matrix-vector product to
independent f64 factor assembly, verify positive curvature and rigid-translation
invariance, and require a lower implicit objective on a stressed patch than the
old four-sweep Jacobi search. Observed patch objectives were 6.8512e-7 J versus
4.4728e-7 J. Existing pins, sphere, VF/reciprocal VF, self-contact, translation,
overflow/retry and grab-residency checks remain.

## Remaining contact-law issue

The primitive-sum barrier's flat-plane tangential-force defect remains. It was
not hidden by reducing stiffness or by modifying CPU physics. The matched runs
show that incomplete coupled convergence was sufficient to explain the reported
default-garment distortion; they do not repair or validate that separate contact
law. See gpu-protrusion-followup.md and its planar oracle for the open research
gate. Friction, canonical IPC weighting/mollification and arbitrary dense-fold
qualification are not established by this fix.

## Completed validation and rejected vector fusion

The separate long regression passed (264.78 s complete test time). Its repeat
reported CPU 21.174% extension / 2.041 mm seam gap, IPC 19.954% / 2.177 mm /
5.787 mm RMS difference, and AL 21.839% / 2.335 mm / 9.267 mm RMS difference.
All eight intermediate/final GPU separation audits passed. The AL repeat differs
from the standalone run, consistent with the documented finite-budget/contact
insertion-order sensitivity; it still passed the fixed quality limits. These
are not byte-identical or fully converged solutions.

A further optimization fused CG vector updates and reductions into one
workgroup while retaining the parallel sparse matvec. Trajectory checks passed
on the small patch, but warm medians only improved 105.607→102.174 ms (IPC)
and 106.238→103.060 ms (AL), roughly 3%, in one local 16-vertex probe.
That was insufficient evidence to retain the added scheduling complexity.
The default keeps the validated separate passes. Raw results are in
coupled-vector-fusion-study.json; the rejected experiment is preserved in
cg-vector-fusion-rejected.patch (apply with git apply --unidiff-zero against
fcf14a6 for research only). No speedup is claimed for this garment-quality fix.

Latest main 02cee23 (independent accordions/scrollbars) was merged after the
physics validation. Its accordion action bits overlapped the solver controls;
solver actions were moved to independent bits and a regression was added.
The latest UI behavior is retained. Post-merge native/shader/accordion checks
passed (139 tests), along with CPU-only all-target compilation and Clippy.
GPU solver sources are unchanged from the fully validated correction.

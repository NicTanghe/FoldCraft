# Shared WGSL contact throttling investigation

FC-0042, 11 November 2026. Owner reports persistent, implausible T-shirt
protrusions in both WGSL backends that do not appear with the original solver.
Default settings and Reset were confirmed. This report is a defect reproduction
request, not merely a missing dense-garment benchmark.

## Confirmed algorithm defect

The original WGSL CCD pass computes one conservative fraction for every
simultaneous Jacobi proposal. Its minimum comes from any near-contact pair,
including nearly tangential motion. Applying that fraction globally can throttle
a disconnected piece. This is unnecessary coupling of numerical convergence,
not a physical interaction.

The correction first shortens each vertex's proposal using all its incident
safety candidates. Each vertex gets radius 0.4*(distance-clearance). Every point
of a convex primitive then moves by at most that radius; the sum of both
primitive motions is at most 0.8*(distance-clearance). The entire linear path
retains positive clearance. Analytic sphere constraints use the analogous
triangle bound. Proposals only shorten, preserving existing r+2L search coverage.
Unresolved AL pairs remain in incidence and receive bounds too.

The existing global CCD check remains as a guard. Armijo still evaluates the
complete simultaneous objective. Its slope is scaled with the actual proposal.
There is no position projection after acceptance. This correction does not fix
slow material Jacobi modes, primitive-pair barrier overcounting or artificial
resistance to tangential motion inside the contact region.

## Falsifiable regression and observed result

The opt-in hardware test close_contact_does_not_throttle_disconnected_free_fall
uses two disconnected small panels. The lower panel is near a floor; the upper
panel is separated. Acceleration is tangent to the floor. A soft membrane and
tiny positive contact stiffness deliberately isolate motion bounds from elastic
Jacobi stiffness and repulsion. This is not the default T-shirt.

With eight sweeps, maximum error against the upper panel's analytic accelerated
motion was:

| Method | Global bound | Per-vertex bounds |
|---|---:|---:|
| IPC | 2.45459 mm | 0.01333 mm |
| Barrier AL | 2.45462 mm | 0.01333 mm |

The test retains an ablation of the previous path, requires the remote stall
there, checks its removal within 20 micrometres, and independently validates
surface separation. All six external opt-in GPU tests passed after the change,
including freefall, pins, sphere impact, VF, reciprocal VF, self-contact,
translation, transactional failures and line-search-path comparisons.

## Full garment status

This proves and corrects a shared throttling defect. It does **not** establish
that this is the sole cause of the pictured shoulder/side protrusion.

A longer matched default-T-shirt comparison was requested, up to 200 steps per
new solver. That GPU workload requires coordination beyond the earlier small
checks. Pending permission, do not describe the pictured defect as fixed.
Use identical reset geometry, material, timestep and initial x/v/time, preserve
the source document, and save final OBJ and residual/contact metrics to separate
output directories. Compare against the original CPU solver and inspect the
reported protrusion, not just nonintersection.

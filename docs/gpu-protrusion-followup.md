# Persistent default-T-shirt protrusion: unresolved

11 November 2026. The owner retested and confirmed that the protrusion remains
after the local-contact-bound correction. Do not mark the visual defect fixed.

## Confirmed force-law defect

Both resident backends use a sum of full finite-primitive barriers. The
CPU-only oracle scripts/research-ipc-planar-force.py evaluates their actual
squared-distance scalar barrier over a two-triangle plane. At constant 1 mm
height with a 3 mm activation radius and 100000 N/m stiffness:

- Minimum energy: 0.1953088513 J.
- Maximum energy: 0.3906177026 J.
- Maximum artificial tangential force: 91.51395238 N.

[Full samples](ipc-planar-force-study.json) are retained. The face containing
the projected point contributes a constant barrier; its neighbor adds an
edge-distance barrier whose gradient points sideways. Thus a flat frictionless
surface artificially resists sliding. Contact splitting has the same underlying
fixed-point energy, so AL cannot correct this force-law error by converging.

This is consistent with the earlier rejected finite-primitive experiment in
primitive-contact-results.md. It is a confirmed mathematical defect, not proof
that it alone causes the photographed protrusion.

## Reproduction capture correction

The native fitting Save button previously saved self.project's launch-time
state cache while the displayed accepted state advanced separately in scene.
It now saves a cloned snapshot with displayed x/v/time and current simulation
settings, then retains it only on successful save. Material rest geometry and
file version are unchanged. A real native headless save/load regression passes.

Existing files saved by the old button do not necessarily contain the visible
bad shape. The available default document has 2454 vertices but only 0.0333 s
of cached simulation. Do not present that cache as the owner's long-run state.

## Next validation and repair

The shared global-bound defect is separately proven and corrected, but its
regression cannot qualify this garment. A correction to contact ownership and
surface force construction needs to pass the flat-plane slide gate, feature
transition tests, self-contact and multi-surface/wedge cases. Selecting only one
globally nearest face would incorrectly discard independent simultaneous walls;
suppressing duplicate records alone also does not remove neighboring edge
barriers. Do not patch this by reducing physical stiffness or adding projections.

The requested longer GPU comparison (<=200 steps per new solver on the default
garment) is still awaiting explicit workload permission; earlier authorization
covered small GPU checks. Preserve the source project and use separate outputs.
A captured protruded state allows CPU force decomposition and restart comparisons
without inventing a visual success metric. The original solver provides the
control, with its different contact law clearly recorded.

## CPU control completed; CPU implementation unchanged

A separate reset copy of the default saved T-shirt was run through the existing
CPU VBD/OGC control for 200 steps, two threads. The 2454-vertex / 4570-triangle
control reached 3.3333 s, maximum seam gap 2.041 mm and maximum extension 21.174%.
[Recorded metrics](protrusion-cpu-control.json) are retained; local geometry is
under output/protrusion-repro/cpu-control. Original user files were preserved.
No CPU solver, material energy, OGC force model or CPU settings were edited.
The only runtime code correction in this follow-up is the graphical Save path.

Further source consultation: the official IPC Toolkit releases describe an
improved max approximator and area weighting
(https://github.com/ipc-sim/ipc-toolkit/releases). The maintainer's issue #133
(https://github.com/ipc-sim/ipc-toolkit/issues/133) documents that combining signed
corrections with EE mollification without area weighting can produce negative
potentials. This rules out treating a simple subtraction of duplicate edge
barriers as a validated drop-in repair. The convergent-formulation tutorial URL
could not be fetched. No external implementation was copied or enabled.

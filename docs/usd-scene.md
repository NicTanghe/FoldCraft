# USD scene boundary

FC-0030, 10 November 2026. This first slice implements **CPU-only static garment
scene publication**. It does not complete the internal USD migration, register a
viewport, import USD, or change the supported editable document formats.

## What cssimpler actually uses

Inspected `NicTanghe/cssimpler`, branch `3D_implementation`, at
[`6ad81a0cef198d757575a50ede312f966d4eb7b9`](https://github.com/NicTanghe/cssimpler/tree/6ad81a0cef198d757575a50ede312f966d4eb7b9).
This differs from FoldCraft's currently pinned cssimpler revision. No dependency
pin or migration code is changed by this slice.

- [`docs/gpu-viewports.md`](https://github.com/NicTanghe/cssimpler/blob/6ad81a0cef198d757575a50ede312f966d4eb7b9/docs/gpu-viewports.md)
  documents genuine USD stages as authoritative scene data. Generic wgpu
  callbacks are also available; they do not replace the built-in USD path.
- [`renderer/Cargo.toml`](https://github.com/NicTanghe/cssimpler/blob/6ad81a0cef198d757575a50ede312f966d4eb7b9/renderer/Cargo.toml)
  enables `openusd` and `openusd-schemas` through optional `gpu-viewport`.
  Its lockfile resolves both to **0.8.0**. This is the
  [mxpv native Rust implementation](https://github.com/mxpv/openusd), rather than
  an FFI binding to Pixar's C++ runtime. Pixar OpenUSD is the independent oracle
  used by the validation script in this change.
- [`renderer/src/gpu/usd.rs`](https://github.com/NicTanghe/cssimpler/blob/6ad81a0cef198d757575a50ede312f966d4eb7b9/renderer/src/gpu/usd.rs)
  defines cloneable `UsdScene` as a native `Stage` plus a shared revision cell.
  `UsdScene::open` installs the standard schema registry;
  `UsdScene::from_stage` accepts an existing stage. `stage()` returns that stage.
  Change sinks invalidate derived geometry and request GPU redraws. The handles
  contain `Rc`, so stages stay on the event thread rather than crossing the
  simulation-worker boundary.
- `UsdRenderer` derives vertex buffers and camera uniforms from the stage. Its
  current geometry path accepts visible convex polygon cages, world transforms,
  and constant display color. It does not evaluate production material networks,
  textures, authored lights, subdivision, or skinning. Orbit cameras are prims
  in a separate transient USD stage; authored cameras use the scene stage.

Those observations establish the compatible scene boundary; they are not a
claim that FoldCraft's current viewport already consumes USD. The implementation
here is original Rust and original fixtures informed by the public format
documentation; it copies no cssimpler or OpenUSD implementation or assets.
Existing project and dependency notices remain applicable.

## Implemented static publication

`usd_scene::garment_stage_usda(&CompiledGarment, &State)` validates a snapshot
and returns a complete USDA string. It performs no I/O, rendering or simulation.
The example publishes the template's compiled initial arrangement:

```powershell
cargo run --locked --no-default-features -j 2 --example usd_static_scene -- examples/templates/two-panel.json output/two-panel.usda
```

Create the output directory first. The example accepts a template JSON and an
output with the `.usda` extension. It does not accept fitting-project JSON or
implicitly recover saved drape state. Library callers can provide a current
accepted state with matching topology. Validation completes before the example
writes a file; a subsequent I/O error is not an atomic file-replacement guarantee.

| USD property | Published meaning |
|---|---|
| Root metadata | Explicit `metersPerUnit = 1`, `upAxis = "Y"`, `defaultPrim = "World"` |
| `/World/Garment` | Scope containing one `Mesh` per compiled panel; custom integer `clothcraft:snapshotVersion = 1` |
| `clothcraft:timeSeconds` | Custom double snapshot timestamp in seconds; this is not a time-sampled animation or playback range |
| Mesh prim name | `panel_` followed by the lowercase hexadecimal UTF-8 bytes of the panel ID; injective and stable across panel reordering |
| `clothcraft:panelId` | Original semantic panel ID as a custom string; mesh indices never become editable IDs |
| `points` | World-space current positions, `point3f[]`, in metres; explicit f64 to f32 conversion |
| `faceVertexCounts`, `faceVertexIndices` | Triangles and panel-local transient point indices, preserving compiler winding |
| `subdivisionScheme`, `orientation`, `doubleSided` | Explicit `none`, `rightHanded`, `true`; polygon cloth without implicit Catmull-Clark or invented thickness |
| `clothcraft:restUV` | Custom `double2[]` in **local material metres**, retained directly from `Mesh.uv`; independent of drape, placement and board layout |
| `primvars:st` | Standard `texCoord2f[]` with `vertex` interpolation, a f32 presentation copy of the same local metre coordinates; no atlas packing or [0,1] normalization |
| `primvars:displayColor` | One constant illustrative color, not fabric appearance or a calibrated material |

Standard `points` are f32 to match UsdGeomMesh and the inspected cssimpler
reader. Solver arrays remain f64. Round-off in the display geometry is expected;
the exact f64 rest data never comes from those points. USD does not assign
physical units to arbitrary custom attributes: the metre/second conventions
above are the explicitly versioned ClothCraft snapshot contract.

The writer rejects missing, overlapping or incomplete panel ranges, duplicate or
empty IDs, out-of-panel triangle references, repeated indices, invalid time,
non-finite values and f32 overflow. It does not certify collision separation,
mesh quality, equilibrium or physically nondegenerate deformed faces. Existing
compiler validation and solver contracts still apply.

This snapshot omits the avatar, seams, named boundaries, fabric IDs/properties,
grain, pins, trims, complete editable contours, placement commands, undo history,
velocities, textures, cameras and authored lights. It therefore cannot reconstruct
an editable project or resume a simulation. No USD/USDC/USDZ reader or general
USD composition implementation is added. Loading this valid USDA in cssimpler's
experimental USD viewport is the intended next integration, not an observed
native-window result in this slice.

## Canonical internal scene target and remaining work

The owner requires USD as the internal scene representation, including the
viewport. The following is the integration target, **not implemented here**:

1. Own one `openusd::usd::Stage` on the application/event thread, registered with
   the matching schemas and shared through cssimpler's `UsdScene`. Geometry,
   scene transforms, cameras, visual materials and lights live in USD. GPU
   buffers, draw lists and acceleration structures are disposable caches, rather
   than a second independently editable scene graph.
2. Preserve `GarmentTemplate` v1/v2, `FittingProject` v1 and stable semantic
   panel/boundary/material references at the compatibility boundary. Design a
   lossless USD authoring schema/layer for those domain values before migrating
   their source of truth. A mesh snapshot is insufficient. Material rest geometry
   remains f64, two-dimensional and expressed in metres; it must never be
   inferred from displayed 3D points. Existing format round trips remain tested.
3. Compile validated authoring data into the current solver arrays. Physics
   modules remain independent of USD/UI/wgpu. Send worker commands and accepted
   snapshots as plain Rust values with scene generation and topology identity;
   author accepted presentation updates on the event thread. Discard stale
   generations before publishing, and group coherent topology/position edits.
   The worker clock remains independent of rendering.
4. Keep persistent authored scene opinions separate from transient simulation
   samples and view-local camera/selection opinions. Define edit targets,
   undo/redo, rejected-transaction rollback, dirty/save semantics and asset
   resolution before exposing editable stage mutations. Stage change notices
   invalidate only derived caches; a render redraw cannot change physics.
5. Implement bounded static USD import separately: explicit units/up-axis,
   transforms, mesh topology, missing resources and unsupported schemas must be
   inspected before installation. Visual mesh import does not recover sewing
   patterns or establish cloth physical properties. Then connect the native
   viewport and validate picking, authoring history, save/reload and cancellation.

Future stage ownership, dependency updates, canonical authoring-schema migration,
static import and native viewport integration remain follow-up work beyond this
first FC-0030 publication slice. The
separate cssimpler-only migration and its all-features dependency gate remain
incomplete until their own acceptance succeeds.

## Validation

The normal Rust tests remain CPU-only:

```powershell
cargo test --locked --no-default-features -j 2 --test usd_scene -- --test-threads=2
```

They cover panel-local topology/winding, semantic ID stability and escaping,
unchanged f64 material coordinates after deformation, display precision,
inconsistent partitions, and invalid snapshots. The optional independent reader
uses Pixar's Python `pxr` API; no GPU or native window is created:

```powershell
# Install separately in an isolated Python environment; never required by CI.
python -m pip install usd-core==26.11
python scripts/check-usd-scene.py tests/fixtures/usd/independent-panel.usda
python scripts/check-usd-scene.py output/two-panel.usda --template examples/templates/two-panel.json
```

The first input is an independently authored, deliberately deformed panel. The
second must be actual output from the example, not a substituted fixture.
`--template` additionally checks rectangular source dimensions, point/face counts,
f64 rest coordinates and analytically transformed placement against the source
JSON. Its geometry oracle deliberately refuses polygon templates and templates
with authored world pin targets or frozen panels, whose compilation can override
placement positions; without that option, it checks the bounded USD scene
contract, topology and UV cardinality.
Missing `pxr` exits with status 2 and a clear dependency message; it is not a pass.
This script is not a complete USD compliance validator or a DCC visual acceptance
test. Observed local execution results belong in the PR.

`python scripts/test_usd_scene_validator.py` opts into the same Pixar dependency
and exercises the independent fixture plus nine invalid mutations: out-of-range
indices, face counts, wrong or missing units, missing version/time/rest UV/points,
and malformed syntax. Each invalid input must exit 1 without a Python traceback.

## Format references consulted

- [Pixar UsdStage](https://openusd.org/release/api/class_usd_stage.html): a
  composed scene view and edit/save ownership, distinct from a GPU mesh cache.
- [UsdGeomMesh](https://openusd.org/release/api/class_usd_geom_mesh.html): point
  and face topology, subdivision defaults, winding and faceted normals.
- [Stage linear units](https://openusd.org/release/api/group___usd_geom_linear_units__group.html):
  explicit metre metadata; the unauthored fallback is centimetres.
- [UsdGeomPrimvar](https://openusd.org/release/api/class_usd_geom_primvar.html):
  typed primvars and interpolation metadata.
- [Hello World stage tutorial](https://openusd.org/release/tut_helloworld.html):
  Pixar `pxr.Usd`/`UsdGeom` reader and stage API, documented against USD 26.11.

These references define the bounded original implementation. They do not imply
support for every OpenUSD schema, Hydra renderer, asset resolver or DCC extension.

## Native viewport rendering

Both cssimpler editors use the registered USD viewport as their sole 3D renderer.
The previous CPU-projected SVG triangle overlay has been removed; the 2D pattern
boards still use SVG. This removes duplicate mesh projection and DOM construction
on UI rebuilds. The existing USD snapshot is still loaded at application creation;
live simulation updates and worker scheduling are separate follow-up work.

Native USD host elements have explicit flex sizing and minimum drawable dimensions;
unlike an SVG, their mesh content does not supply an intrinsic DOM size.

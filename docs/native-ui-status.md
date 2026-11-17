# Native UI status after the egui removal

Audit: 17 November 2026, against `main` at `fcd319a`. FC-0022 ([NicTanghe/FoldCraft#32](https://github.com/NicTanghe/FoldCraft/pull/32), commit `2fa571d`) replaced the egui/eframe desktop UI with cssimpler and direct wgpu, and deleted the egui adapters under `src/atelier/`. The CPU model behind them (`authoring`, `garment`, `document`, `fitting`, `fabrics`, `pattern_import`, `avatar_import`, `template_catalog`, solvers) is unchanged. Many older documents describe egui-era controls. This page records what the native `cloth-fit --view` workspace actually wires up, so contributors don't rely on a control that no longer exists.

"Wired" means a native control issues the real command or action. "Display only" means the panel shows values but has no editing path. "Not ported" means only the CPU API remains.

## Where the UI lives now

| Responsibility | Location |
|---|---|
| `cloth-fit --view` workspace: layout, event handling, 2D board drawing/picking, tool modes, property accordions | `src/atelier/css_runner.rs` |
| UI-independent workspace state, command registry facade and worker channels | `src/atelier/core_state.rs`, `src/atelier/commands.rs`, `src/workspace_commands.rs` |
| Simulation worker, fixed-step clock and generation filtering | `src/atelier/worker.rs` |
| Stylesheet, theme tokens, icons, viewport compositor and input routing | `src/atelier/css_ui/mod.rs`, `studio_v2.css`, `icons.rs`, `compositor.rs` |
| Top bar, tool rails and command palette components | `src/atelier/css_ui/rails.rs`, `palette.rs` |
| Board navigation (pan/zoom), independent of the UI | `src/viewport_controller.rs` |
| `marvelous-cloth` Project v1 window, including mesh preparation | `src/editor/css_editor.rs` |

`css_ui/browsers.rs`, `css_ui/properties_part1.rs`, `css_ui/properties_part2.rs` and the radial menu in `css_ui/rails.rs` are standalone components with their own tests. `css_runner.rs` does not mount them; it builds its own panels inline. Their import, profile, trims and diagnostics controls are therefore not reachable in the running workspace.

## Wired in `cloth-fit --view`

- 2D board drawing: pen polygon, primitives, freehand panels, internal lines, linked mirrors, anchor moves, multi-point moves, point removal, curve toggling, boundary splits, line removal and the Delete key for selected points.
- Corner tools: single and batch fillets with live preview, and corner handle adjustment (`authoring/corner_tool.rs`).
- 2D sewing (`Command::Sew`) and seam removal. The second boundary click commits a whole-edge seam (`range: [0, 1]`, not reversed) with `ease: true`, without the egui-era confirmation step. Unequal lengths are therefore eased implicitly rather than by an explicit user choice.
- Saved measurements (`Command::SetMeasurement`).
- Undo/redo for these 2D edits.
- Play/pause, step, reset, the solver dropdown, clean view, light/dark theme and the command palette.
- Loading the bundled T-shirt or dress preset (the Apron entry reports that it is planned).
- **Save Project (Ctrl+S)** writes `<--output>/fitting-project.json`, including the state and settings. It goes through `FittingProject::save`, so the write is atomic and the avatar is copied into the adjacent `.assets` folder, but it never writes to the file passed to `--project`.
- Export of OBJ/SVG artifacts to the output directory.
- 3D viewport orbit and zoom.
- `--ui-smoke` writes `ui.png`; `--ui-perf-smoke` writes `ui-perf.json`.

## Display only

- Panel placement (translation/rotation) in the Arrange accordion.
- Pin count and mode in the Pins accordion.
- The **Fabric Mechanics** accordion shows fixed values (180 g/m², 150/120 N/m, 0.002 N·m) for every panel. Only the preset name comes from the document. The values are not the panel's compiled properties.
- 3D rail modes Gizmo, Grab, 3D Sew, Pin, Measure and Avatar Diagnostics. Each one only sets a status label.

## Not ported (CPU API only)

| Capability | Removed egui adapter | CPU API that remains |
|---|---|---|
| Open, Save As, dirty-state prompts, 60 s recovery | `documents_ui.rs` | `document.rs`, `fitting.rs` |
| DXF / SVG pattern import | `pattern_import_ui.rs`, `svg_import_ui.rs` | `pattern_import.rs`, `garment/dxf_import.rs`, `garment/svg_import.rs` |
| Avatar import (OBJ/glTF/GLB) | `avatar_import_ui.rs` | `avatar_import.rs`, `gltf_avatar.rs`; `cloth-fit --avatar` |
| Six-template browser with previews | `template_browser.rs` | `template_catalog.rs` |
| Fabric assignment | `fabrics_ui.rs` | `fabrics.rs`, `Command::Fabric` |
| 3D placement gizmo, group placement, 2D board layout transform | `gizmo.rs`, `selection.rs` | `Command::{Place, PlaceGroup, BoardTransform, LayoutGroup}` |
| 3D edge picking and sewing, seam-link overlays | `edges_3d.rs`, `seam_links.rs`, `sewing_ui.rs` | `garment/seam.rs`, `Command::{Sew, SewChain}` |
| Grab brush | egui-era `atelier.rs`, `chrome.rs` | `picking.rs`, `vbd::Grab` |
| Box/lasso selection, semantic clipboard | `selection.rs` | `authoring/clipboard.rs`, `Command::Paste` |
| Radial tool menu, Q quick actions | `radial.rs`, `quick_actions.rs` | — |
| Booleans, independent mirrors, ruffles, cut-and-sewn darts, synchronized cutouts | `drafting_ui.rs`, `authoring_ui.rs` | `Command::{Boolean, Mirror, Ruffle, DartCutSew, SynchronizedCutout}` |
| Pins editor, trims, seam allowances/notches editing | `pins_ui.rs`, `trims_ui.rs`, `authoring_ui.rs` | `garment/pins.rs`, `garment/trims.rs`, `garment/markings.rs` |
| Sewing audit panel | `sewing_ui.rs` | `cloth-template audit-sewing` |
| Avatar collision diagnostics panel | `avatar_diagnostics.rs` | `avatar::diagnostics` |
| Issue capture painting | `issue_ui.rs` | `issue_report.rs` |
| Presentation/lighting controls, preferences, resizable panes, workspace profiles | `stage_settings.rs`, `settings.rs`, `layout.rs`, `workspace_profiles.rs` | `render/` |
| Mesh preparation in `cloth-fit` | `mesh_resolution_ui.rs` | `mesh_resolution.rs` (wired in `marvelous-cloth` only) |
| Named-widget CPU harness and scripted authoring smoke | `authoring_smoke.rs`, `authoring_smoke/widget_harness.rs` | — |

`--ui-interaction-smoke` and `--ui-authoring-smoke` are still accepted by `cloth-fit`, but `css_runner.rs` stores the flags without acting on them. They open the window and no longer write `interaction.json` or `authoring.json`. The native window also has no application icon set.

## Platform limits

On Linux, cssimpler's GPU viewport only embeds into X11 windows and fails on Wayland with *"Linux GPU viewports currently require X11"*. Until FC-0065 lands, start the window with `env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET` to use XWayland. FC-0065 makes the binaries do that automatically when `DISPLAY` is available (`FOLDCRAFT_KEEP_WAYLAND=1` opts out); see `docs/install.md`.

## Reading older documents

Validation records, PR reviews and dated handoff entries keep their egui references, because they describe what was tested at the time. When a guide names a removed module or a menu path such as **File → Import…**, check the tables above before assuming the control exists.

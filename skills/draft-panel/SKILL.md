---
name: draft-panel
description: Add a panel to a ClothCraft garment from a polygon in material-space metres, assign a fabric, place it in 3D, and save a native template.
---

# Draft a panel

A panel is rest geometry in metres, drafted on a 2D board. Never derive it from
a drape, a screenshot or a simulated shape.

## Steps

1. `document_open {"catalog": "tshirt"}` (or `path` for a saved document).
2. `document_inspect {}` and keep `document.fabrics[].name`. A fabric name that
   is not in the list needs explicit properties.
3. Add the panel:
   ```json
   garment_add_panel {
     "points": [[0.00,0.00],[0.42,0.00],[0.42,0.58],[0.00,0.58]],
     "fabric": "Uncalibrated woven reference",
     "target_edge_m": 0.025
   }
   ```
   - `points` are `[u,v]` in metres, counterclockwise, at least three.
   - `target_edge_m` is the nominal triangle edge size; the accepted range is
     3 mm to 500 mm, and the budget is 64 to 100,000 vertices.
   - `min_angle_degrees` must stay within 15 to 30.
   - Curves are cubic: pass `tangents` with one control point per vertex.
     Omitting them drafts straight edges.
4. Place it in the world:
   `garment_place {"panel": "<new id>", "translation": [0, 0, 1.42], "rotation_degrees": [0, 0, 0]}`
   Translation is metres; rotation is XYZ Euler degrees.
5. Re-read with `document_inspect {}` and confirm the new panel and its
   boundaries appear.
6. Save when the document should persist:
   `document_save {"path": "/abs/path/garment.json"}`

## Rules

- Material coordinates are the rest shape. A placement is a rigid transform of
  that shape and never new rest geometry.
- The drafting board position (`garment_layout`) is presentation only.
- The panel id is chosen by the engine; read it back, do not assume it.

## Refusals and what they mean

| code | meaning |
|---|---|
| `invalid_geometry` | self-intersecting or degenerate contour, or meshing limits violated |
| `unknown_fabric` | no fabric by that name; pass `properties` |
| `no_document` | open a document first |
| `stale_revision` | something else changed the document; re-inspect and retry |

## Verify

- `document.counts.panels` grew by exactly one.
- The new panel's `boundaries` have the names you expect to sew.
- `revision` advanced once for the add and once for the placement.

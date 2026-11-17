---
name: sew-seams
description: Sew two named boundaries on a ClothCraft garment, choosing direction and easing deliberately, then verify the seam graph.
---

# Sew seams

Sewing is a relationship between named boundaries. It is not a pinned pair of
vertices, and it is not a weld: the two halves stay editable.

## Steps

1. `document_inspect {}` and pick two boundaries by the names it reported.
2. Sew:
   ```json
   garment_sew {
     "a": {"panel": "front", "edge": "shoulder-right"},
     "b": {"panel": "back", "edge": "shoulder-right"},
     "reverse": false,
     "ease": false
   }
   ```
3. Read the result: `document.counts.seams` must grow by one, and the response
   lists the new seam with its two boundaries.

## Direction

- Boundaries have a direction along the contour. If the seam twists, set
  `"reversed": true` on one side — that flips that boundary, it does not mirror
  the panel.
- `reverse` on the tool flips the whole correspondence; prefer `reversed` per
  side so the intent stays local and readable.

## Unequal lengths

- Without `ease`, two boundaries of different length are a mismatch to fix by
  editing geometry — the engine will not silently stretch one to the other.
- With `"ease": true` the seam permits unequal lengths: gathering and easing.
  Use it deliberately, and say so, because it changes what the seam means.

## Part of a boundary

- Pass `"range": [0.25, 0.75]` to sew a normalized arclength interval rather
  than the whole boundary. Intervals are positions along the boundary, not
  vertex indices.

## Removing

`garment_remove_seam {"seam": "<id>"}` — ids come from `document_inspect`.

## Refusals and what they mean

| code | meaning |
|---|---|
| `invalid_geometry` | the boundaries do not exist, or the correspondence is not monotone |
| `stale_revision` | re-inspect; another change landed first |
| `no_document` | nothing is open |

## Verify

- `document.counts.seams` is exactly one higher than before.
- The new seam's `a` and `b` name the boundaries you meant, with the ranges you
  passed.
- Undo once (`garment_undo`) returns the count and the geometry to the start.

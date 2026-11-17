---
name: inspect-document
description: Read a ClothCraft garment document before touching it — panels, their stable boundary names, seams and fabrics — so every later edit targets something real.
---

# Inspect the document

Use this before any edit. An edit aimed at a guessed identifier is the most
common way to lose time here.

## Steps

1. Open something. Either a bundled engineering template:
   `document_open {"catalog": "tshirt"}`
   or a native template on disk:
   `document_open {"path": "/abs/path/garment.json"}`
   Opening replaces the resident document and resets history.
2. Read it back:
   `document_inspect {}`
   The answer carries `revision`, `canUndo`, `canRedo` and a `document` summary.

## What the summary gives you

- `document.panels[].id` — the only panel identifiers that exist. Never invent one.
- `document.panels[].boundaries[]` — the stable boundary names. For a polygon
  panel these are its named edges; for a legacy rectangle panel they are
  `top`, `right`, `bottom`, `left`.
- `document.seams[].id` — existing seams, each with its `a` and `b` boundaries.
- `document.fabrics[].name` — fabric names already in the document, usable by
  `garment_fabric` without passing properties.
- `document.panels[].placement` — metres and XYZ Euler degrees, world space.
- `document.panels[].layout_2d` — board position. Presentation only; it never
  moves rest geometry.

## Rules

- Every identifier comes from this answer, not from memory of a prior session.
- `revision` is the document's version. Any later mutation must not assume it
  stuck: re-read after a failure.
- Boundaries and panels are stable names; mesh vertex indices are not part of
  this surface and must not be used.

## Verify

- The panel you plan to edit appears in `document.panels`.
- The boundary you plan to sew appears in that panel's `boundaries`.
- `revision` is the one you will act on.

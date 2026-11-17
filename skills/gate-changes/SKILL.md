---
name: gate-changes
description: Propose a change without applying it, show a preview, and let a person confirm — the workshop/record gate for ClothCraft documents.
---

# Gate a change

Use this whenever a person should see a change before it lands, or whenever you
want to inspect the compiled result of an edit before committing to it.

## The three calls

1. Propose:
   ```json
   change_prepare {"command": {"kind": "layout", "panel": "front", "origin": [0.4, 0.3]}}
   ```
   The answer carries `change` (a token), `revision` (unchanged) and
   `preview` (a summary of the document that would result). Nothing is applied
   and the undo history is untouched.
2. Decide, out loud, for a person: the change is good, or it is not.
3. Apply or discard:
   - `change_commit {"change": "change-1"}`
   - `change_cancel {"change": "change-1"}`

## Rules

- One pending change at a time. A second `change_prepare` before committing or
  cancelling is refused with `pending_change`.
- A commit advances the revision by one; a cancel does not move it.
- Any direct edit, undo, redo or document replacement invalidates the token.
- The token is not an authorization: it only identifies a computed change.

## Why this exists

The workshop is where an agent may generate freely; the record is small and
versioned, and only a person promotes work into it (see the project's own rule:
an agent may never lock). `change_prepare` is the agent's proposal,
`change_commit` is the person's decision.

## Verify

- Before commit: `document_inspect` shows the same `revision` as before prepare.
- After commit: `revision` is one higher and `canUndo` is true.
- After cancel: the document is byte-for-byte the one you prepared against.

# Local studio API

The React studio never owns the document. It reads and mutates a resident
clothcraft.authoring session through `cloth-server`, the same session the MCP
server drives. One document, two doors: an agent and a person cannot drift into
different states.

Loopback only, no authentication, one resident document. This is the editor's
own socket, not a service.

## Run

```sh
cargo run --no-default-features --bin cloth-server      # 127.0.0.1:8787
cd studio && npm install && npm run dev                 # 127.0.0.1:5173
```

Vite proxies `/api` to the server, so the browser stays on one origin during
development. `CLOTH_SERVER_ADDR` moves the socket.

## Endpoints

| Method | Path | Body | Result |
|---|---|---|---|
| GET | `/api/health` | — | service name and version |
| GET | `/api/catalog` | — | bundled engineering templates |
| POST | `/api/open` | `{"catalog": id}`, `{"path": path}` or `{"template": {...}}` | document summary |
| GET | `/api/document` | — | document summary |
| GET | `/api/preview` | — | board-space polylines in metres, with bounds |
| POST | `/api/command` | `{"command": {...}}` | document summary |
| POST | `/api/prepare` | `{"command": {...}}` | change token, preview, no mutation |
| POST | `/api/commit` | `{"change": token}` | document summary |
| POST | `/api/cancel` | `{"change": token}` | document summary |
| POST | `/api/undo` | — | document summary |
| POST | `/api/redo` | — | document summary |

Rejected requests answer 400 with `{"error": {"code", "detail"}}` and leave the
document, the pending change and the history untouched.

## What the preview is, and is not

`/api/preview` returns polylines from `template_catalog::Selection`, the same
board-space paths the desktop chrome uses. They are metres on the unified 2D
drafting board: no draped position and no renderer coordinate enters them.
Internal lines are marked so a viewer can dash them.

Edges keep their stable names here; that is why the studio can name a boundary
it wants to sew. A preview is not a compiled mesh, and it is not simulated
state.

## Missing on purpose

There is no simulation endpoint yet. Stepping the solver, streaming draped
vertices and exporting artefacts are separate slices; until they land the studio
shows the drafting board and the document, not a drape.

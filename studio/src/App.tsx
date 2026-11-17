import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, type CatalogEntry, type DocumentSummary, type Panel, type Preview } from "./api";

const PANEL_COLOURS = ["#c8452b", "#2f6f6a", "#8a6b2f", "#4a5b8c", "#7a4a6a", "#3f6b3f"];

type Selection = { panel: string | null };

export default function App() {
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [summary, setSummary] = useState<DocumentSummary | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [selection, setSelection] = useState<Selection>({ panel: null });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const [document, drawing] = await Promise.all([api.document(), api.preview()]);
    setSummary(document);
    setPreview(drawing);
  }, []);

  const open = useCallback(
    async (id: string) => {
      setBusy(true);
      setError(null);
      try {
        await api.open(id);
        await refresh();
        setSelection({ panel: null });
      } catch (problem) {
        setError(problem instanceof Error ? problem.message : String(problem));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { templates } = await api.catalog();
        if (cancelled) return;
        setCatalog(templates);
        const first = templates.find((entry) => entry.id === "tshirt") ?? templates[0];
        if (first) await open(first.id);
      } catch (problem) {
        if (!cancelled) {
          setError(
            problem instanceof Error
              ? `${problem.message} — is cloth-server running?`
              : String(problem),
          );
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open]);

  const history = useCallback(
    async (action: "undo" | "redo") => {
      setBusy(true);
      try {
        action === "undo" ? await api.undo() : await api.redo();
        await refresh();
      } catch (problem) {
        setError(problem instanceof Error ? problem.message : String(problem));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const document = summary?.document ?? null;
  const panelColour = useMemo(() => {
    const map = new Map<string, string>();
    document?.panels.forEach((panel, index) => {
      map.set(panel.id, PANEL_COLOURS[index % PANEL_COLOURS.length]);
    });
    return map;
  }, [document]);

  return (
    <div className="studio">
      <header className="top-bar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true" />
          <span className="brand-name">FoldCraft</span>
          <span className="brand-stage">Studio</span>
        </div>

        <label className="template-picker">
          <span className="label">Garment</span>
          <select
            value={document ? undefined : ""}
            disabled={busy || catalog.length === 0}
            onChange={(event) => open(event.target.value)}
          >
            {catalog.map((entry) => (
              <option key={entry.id} value={entry.id}>
                {entry.title}
              </option>
            ))}
          </select>
        </label>

        <div className="transport">
          <button type="button" disabled={busy || !summary?.canUndo} onClick={() => history("undo")}>
            Undo
          </button>
          <button type="button" disabled={busy || !summary?.canRedo} onClick={() => history("redo")}>
            Redo
          </button>
        </div>

        <div className="revision" title="Document revision. Every change bumps it.">
          <span className="dot" data-live={Boolean(document)} />
          rev {summary?.revision ?? "—"}
        </div>
      </header>

      <div className="workspace">
        <nav className="tool-rail" aria-label="Tools">
          {[
            ["select", "Select", "V"],
            ["pen", "Pen", "P"],
            ["sew", "Sew", "S"],
            ["move", "Move", "M"],
            ["sim", "Simulate", "Space"],
          ].map(([id, label, key]) => (
            <button key={id} type="button" className="tool" title={`${label} (${key})`} disabled>
              <span className="tool-glyph" aria-hidden="true">
                {label.slice(0, 2)}
              </span>
              <span className="tool-label">{label}</span>
            </button>
          ))}
        </nav>

        <main className="board">
          {preview ? (
            <Board
              preview={preview}
              selection={selection}
              panelColour={panelColour}
              onSelect={(panel) => setSelection({ panel })}
            />
          ) : (
            <div className="board-empty">
              {busy ? "Opening…" : "Open a garment to start."}
            </div>
          )}
        </main>

        <aside className="inspector" aria-label="Inspector">
          <Inspector
            document={document}
            selection={selection}
            onSelect={(panel) => setSelection({ panel })}
          />
        </aside>
      </div>

      <footer className="status-bar">
        <span>{document ? document.name : "No document"}</span>
        <span className="spacer" />
        {preview?.warnings.map((warning) => (
          <span key={warning} className="warning">
            {warning}
          </span>
        ))}
        {error ? <span className="error">{error}</span> : null}
        <span className="units">{preview?.units ?? "metres"}</span>
      </footer>
    </div>
  );
}

function Board({
  preview,
  selection,
  panelColour,
  onSelect,
}: {
  preview: Preview;
  selection: Selection;
  panelColour: Map<string, string>;
  onSelect: (panel: string) => void;
}) {
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number } | null>(null);
  const svg = useRef<SVGSVGElement | null>(null);

  const fit = useMemo(() => {
    const [minX, minY] = preview.bounds.min;
    const [maxX, maxY] = preview.bounds.max;
    const pad = Math.max((maxX - minX) * 0.08, (maxY - minY) * 0.08, 0.02);
    return { x: minX - pad, y: minY - pad, w: maxX - minX + pad * 2, h: maxY - minY + pad * 2 };
  }, [preview]);

  const view = {
    x: fit.x + pan.x,
    y: fit.y + pan.y,
    w: fit.w / zoom,
    h: fit.h / zoom,
  };

  const gridStep = useMemo(() => {
    const target = view.w / 10;
    const steps = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1];
    return steps.find((step) => step >= target) ?? 1;
  }, [view.w]);

  const lines: { x1: number; y1: number; x2: number; y2: number }[] = [];
  const startX = Math.floor(view.x / gridStep) * gridStep;
  const startY = Math.floor(view.y / gridStep) * gridStep;
  for (let x = startX; x < view.x + view.w; x += gridStep) {
    lines.push({ x1: x, y1: view.y, x2: x, y2: view.y + view.h });
  }
  for (let y = startY; y < view.y + view.h; y += gridStep) {
    lines.push({ x1: view.x, y1: y, x2: view.x + view.w, y2: y });
  }

  return (
    <svg
      ref={svg}
      className="board-canvas"
      viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`}
      onWheel={(event) => {
        event.preventDefault();
        const factor = Math.exp(-event.deltaY * 0.0015);
        setZoom((value) => Math.min(24, Math.max(0.25, value * factor)));
      }}
      onPointerDown={(event) => {
        drag.current = { x: event.clientX, y: event.clientY };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (!drag.current || !svg.current) return;
        const rect = svg.current.getBoundingClientRect();
        const dx = (event.clientX - drag.current.x) * (view.w / rect.width);
        const dy = (event.clientY - drag.current.y) * (view.h / rect.height);
        setPan((value) => ({ x: value.x - dx, y: value.y - dy }));
        drag.current = { x: event.clientX, y: event.clientY };
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
    >
      <g className="grid">
        {lines.map((line, index) => (
          <line key={index} {...line} />
        ))}
      </g>
      {preview.paths.map((path, index) => {
        const points = path.points.map(([x, y]) => `${x},${y}`).join(" ");
        const colour = panelColour.get(path.panel) ?? "#555";
        const selected = selection.panel === path.panel;
        return path.closed ? (
          <polygon
            key={`${path.panel}-${index}`}
            points={points}
            className="piece"
            data-selected={selected}
            data-internal={path.internal}
            style={{ "--piece": colour } as React.CSSProperties}
            onPointerDown={(event) => {
              event.stopPropagation();
              onSelect(path.panel);
            }}
          />
        ) : (
          <polyline
            key={`${path.panel}-${index}`}
            points={points}
            className="piece internal"
            style={{ "--piece": colour } as React.CSSProperties}
          />
        );
      })}
    </svg>
  );
}

function Inspector({
  document,
  selection,
  onSelect,
}: {
  document: DocumentSummary["document"];
  selection: Selection;
  onSelect: (panel: string) => void;
}) {
  if (!document) {
    return <div className="inspector-empty">Nothing open yet.</div>;
  }
  const selectedPanel = document.panels.find((panel) => panel.id === selection.panel);
  return (
    <>
      <section className="group">
        <h2>
          Pieces <span className="count">{document.counts.panels}</span>
        </h2>
        <ul className="piece-list">
          {document.panels.map((panel) => (
            <li key={panel.id}>
              <button
                type="button"
                className="piece-row"
                data-selected={selection.panel === panel.id}
                onClick={() => onSelect(panel.id)}
              >
                <span className="piece-name">{panel.name}</span>
                <span className="piece-meta">
                  {panel.fabric} · {panel.boundaries.length} edges
                </span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      {selectedPanel ? <PieceDetail panel={selectedPanel} /> : null}

      <section className="group">
        <h2>
          Seams <span className="count">{document.counts.seams}</span>
        </h2>
        <ul className="seam-list">
          {document.seams.map((seam) => (
            <li key={seam.id}>
              <span className="seam-name">{seam.id}</span>
              <span className="seam-meta">
                {seam.a.edge ?? seam.a.side} ↔ {seam.b.edge ?? seam.b.side}
                {seam.allow_ease ? " · ease" : ""}
              </span>
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}

function PieceDetail({ panel }: { panel: Panel }) {
  return (
    <section className="group detail">
      <h2>{panel.name}</h2>
      <dl>
        <dt>Fabric</dt>
        <dd>{panel.fabric}</dd>
        <dt>Grain</dt>
        <dd>{panel.grain_degrees}°</dd>
        <dt>Boundaries</dt>
        <dd>
          <ul className="tag-list">
            {panel.boundaries.map((boundary) => (
              <li key={boundary}>{boundary}</li>
            ))}
          </ul>
        </dd>
      </dl>
    </section>
  );
}

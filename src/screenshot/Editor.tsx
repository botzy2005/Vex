import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useBackend, useResolvedTheme, useSettings, useSubscription } from '../backend/hooks';
import { canRedo, canUndo, createHistory, push, redo, undo, type History } from './history';
import {
  clampBox, dragBox, resizeFrom, shapeFromDrag, sizeScale, toClientPoint, toImagePoint, validateSize,
  type Box, type Pt, type Shape, type Tool,
} from './model';
import {
  ArrowIcon, BlurIcon, CircleIcon, CopyIcon, CropIcon, HighlightIcon, LineIcon, PixelateIcon, RectIcon, RedoIcon, ResizeIcon,
  RotateLeftIcon, RotateRightIcon, SaveIcon, TextIcon, UndoIcon,
} from './icons';
import { canvasFromImage, cropDoc, docFromCanvas, exportPng, renderDoc, resizeDoc, rotateDoc, type Doc } from './render';

type ToolDef = { id: Tool; label: string; hint: string; icon: typeof CropIcon };
const TOOL_GROUPS: Array<{ label: string; tools: ToolDef[] }> = [
  { label: 'Select', tools: [{ id: 'crop', label: 'Crop', hint: 'Drag the area to keep', icon: CropIcon }] },
  {
    label: 'Annotate',
    tools: [
      { id: 'arrow', label: 'Arrow', hint: 'Drag to draw an arrow', icon: ArrowIcon },
      { id: 'line', label: 'Line', hint: 'Drag to draw a line', icon: LineIcon },
      { id: 'rect', label: 'Rectangle', hint: 'Drag to draw a rectangle', icon: RectIcon },
      { id: 'ellipse', label: 'Circle', hint: 'Drag to draw a circle or oval', icon: CircleIcon },
      { id: 'text', label: 'Text', hint: 'Click where the text should go', icon: TextIcon },
      { id: 'highlight', label: 'Highlight', hint: 'Drag over what to highlight', icon: HighlightIcon },
    ],
  },
  {
    label: 'Hide',
    tools: [
      { id: 'blur', label: 'Blur', hint: 'Drag over anything private to blur it', icon: BlurIcon },
      { id: 'pixelate', label: 'Pixelate', hint: 'Drag over anything private to pixelate it', icon: PixelateIcon },
    ],
  },
];
const TOOLS = TOOL_GROUPS.flatMap((g) => g.tools);
/** Which option controls matter for each tool, so the options row only shows what applies. */
const USES_COLOR = new Set<Tool>(['arrow', 'line', 'rect', 'ellipse', 'text', 'highlight']);
const USES_WIDTH = new Set<Tool>(['arrow', 'line', 'rect', 'ellipse']);

const COLORS = ['#e5252a', '#ff9500', '#ffd60a', '#30d158', '#0a84ff', '#ffffff', '#000000'];

type Status = { text: string; kind: 'ok' | 'error'; path?: string };

export function EditorApp() {
  const backend = useBackend();
  const { settings } = useSettings();
  const theme = useResolvedTheme(settings?.appearance.theme);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const [shotId, setShotId] = useState<number | null>(() => {
    const v = new URLSearchParams(location.search).get('id');
    return v === null || v === '' ? null : Number(v);
  });
  useSubscription<number>((cb) => backend.onEditorLoad(cb), (id) => setShotId(id));

  const [hist, setHist] = useState<History<Doc> | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [tool, setTool] = useState<Tool>('arrow');
  const [color, setColor] = useState(COLORS[0]);
  const [width, setWidth] = useState(4);
  const [textSize, setTextSize] = useState(28);
  const [draft, setDraft] = useState<Shape | null>(null);
  const [crop, setCrop] = useState<Box | null>(null);
  const [textAt, setTextAt] = useState<Pt | null>(null);
  const [resizing, setResizing] = useState<{ w: number; h: number; lock: boolean } | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drag = useRef<{ a: Pt } | null>(null);
  const doc = hist?.present ?? null;

  // ---- load the capture
  useEffect(() => {
    if (shotId === null) {
      setLoadError('No screenshot was chosen.');
      return;
    }
    let alive = true;
    setLoadError(null);
    backend
      .screenshotImage(shotId)
      .then(
        (url) =>
          new Promise<HTMLImageElement>((resolve, reject) => {
            const img = new Image();
            img.onload = () => resolve(img);
            img.onerror = () => reject(new Error('The screenshot could not be read.'));
            img.src = url;
          }),
      )
      .then((img) => {
        if (!alive) return;
        setHist(createHistory(docFromCanvas(canvasFromImage(img))));
        setCrop(null);
        setTextAt(null);
        setDraft(null);
        setStatus(null);
      })
      .catch((e) => alive && setLoadError(String(e instanceof Error ? e.message : e)));
    return () => {
      alive = false;
    };
  }, [backend, shotId]);

  // ---- draw
  useEffect(() => {
    const c = canvasRef.current;
    if (!c || !doc) return;
    if (c.width !== doc.width || c.height !== doc.height) {
      c.width = doc.width;
      c.height = doc.height;
    }
    const ctx = c.getContext('2d');
    if (!ctx) return;
    renderDoc(ctx, doc, draft ? [draft] : []);
    if (crop) {
      // Dim everything outside the part that will be kept.
      ctx.save();
      ctx.fillStyle = 'rgba(0,0,0,0.55)';
      ctx.beginPath();
      ctx.rect(0, 0, doc.width, doc.height);
      ctx.rect(crop.x, crop.y, crop.w, crop.h);
      ctx.fill('evenodd');
      ctx.strokeStyle = '#fff';
      ctx.lineWidth = Math.max(1, sizeScale(doc.width, doc.height) * 1.5);
      ctx.setLineDash([8 * sizeScale(doc.width, doc.height), 5 * sizeScale(doc.width, doc.height)]);
      ctx.strokeRect(crop.x + 0.5, crop.y + 0.5, crop.w, crop.h);
      ctx.restore();
    }
  }, [doc, draft, crop]);

  const commit = useCallback((next: Doc) => setHist((h) => (h ? push(h, next) : h)), []);
  const say = (text: string, kind: Status['kind'] = 'ok', path?: string) => setStatus({ text, kind, path });

  // ---- pointer
  const pointAt = (e: { clientX: number; clientY: number }): Pt | null => {
    const c = canvasRef.current;
    if (!c || !doc) return null;
    return toImagePoint({ x: e.clientX, y: e.clientY }, c.getBoundingClientRect(), doc.width, doc.height);
  };

  const onDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!doc || e.button !== 0) return;
    const p = pointAt(e);
    if (!p) return;
    if (tool === 'text') {
      setCrop(null);
      setTextAt(p);
      e.preventDefault();
      return;
    }
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { a: p };
    setTextAt(null);
    if (tool === 'crop') setCrop(null);
  };
  const onMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d || !doc) return;
    const p = pointAt(e);
    if (!p) return;
    if (tool === 'crop') setCrop(dragBox(d.a, p, doc.width, doc.height));
    else if (tool !== 'text') setDraft(shapeFromDrag(tool, d.a, p, { color, width, imgW: doc.width, imgH: doc.height }));
  };
  const onUp = () => {
    const d = drag.current;
    drag.current = null;
    if (!d || !doc) return;
    if (draft) {
      commit({ ...doc, shapes: [...doc.shapes, draft] });
      setDraft(null);
    }
  };

  // ---- text
  const finishText = (value: string) => {
    const at = textAt;
    setTextAt(null);
    if (!at || !doc || !value.trim()) return;
    commit({ ...doc, shapes: [...doc.shapes, { type: 'text', at, text: value, color, size: textSize * sizeScale(doc.width, doc.height) }] });
  };

  // ---- picture-changing actions (these flatten the drawing into the picture; Undo brings it back)
  const applyCrop = () => {
    if (!doc || !crop) return;
    const box = clampBox(crop, doc.width, doc.height);
    if (box) commit(cropDoc(doc, box));
    setCrop(null);
  };
  const rotate = (turns: 1 | -1) => {
    if (!doc) return;
    setCrop(null);
    commit(rotateDoc(doc, turns));
  };
  const resizeError = resizing ? validateSize(resizing.w, resizing.h) : null;
  const applyResize = () => {
    if (!doc || !resizing || resizeError) return;
    if (resizing.w !== doc.width || resizing.h !== doc.height) commit(resizeDoc(doc, resizing.w, resizing.h));
    setResizing(null);
  };

  // ---- output
  const run = async (what: string, f: () => Promise<void>) => {
    if (!doc || busy) return;
    setBusy(true);
    try {
      await f();
    } catch (e) {
      say(`${what} failed: ${e instanceof Error ? e.message : String(e)}`, 'error');
    } finally {
      setBusy(false);
    }
  };
  const copy = () =>
    run('Copy', async () => {
      await backend.screenshotCopy({ data: exportPng(doc!) });
      say('Copied to the clipboard');
    });
  const save = (ask: boolean) =>
    run('Save', async () => {
      const path = await backend.screenshotSave({ data: exportPng(doc!) }, ask);
      if (path) say(`Saved to ${path}`, 'ok', path);
    });

  // ---- keyboard
  const undoNow = () => setHist((h) => (h ? undo(h) : h));
  const redoNow = () => setHist((h) => (h ? redo(h) : h));
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
      const mod = e.ctrlKey || e.metaKey;
      if (e.key === 'Escape') {
        setCrop(null);
        setDraft(null);
        setTextAt(null);
        setResizing(null);
        drag.current = null;
      } else if (typing) {
        return;
      } else if (mod && e.key.toLowerCase() === 'z' && !e.shiftKey) {
        e.preventDefault();
        undoNow();
      } else if (mod && (e.key.toLowerCase() === 'y' || (e.key.toLowerCase() === 'z' && e.shiftKey))) {
        e.preventDefault();
        redoNow();
      } else if (mod && e.key.toLowerCase() === 's') {
        e.preventDefault();
        void save(e.shiftKey);
      } else if (e.key === 'Enter' && crop) {
        applyCrop();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const toolInfo = TOOLS.find((t) => t.id === tool)!;
  const textBox = useMemo(() => {
    const c = canvasRef.current;
    if (!textAt || !c || !doc) return null;
    const r = c.getBoundingClientRect();
    const p = toClientPoint(textAt, r, doc.width, doc.height);
    return { left: p.x, top: p.y, fontSize: textSize * sizeScale(doc.width, doc.height) * (r.width / doc.width) };
  }, [textAt, doc, textSize]);

  if (loadError) {
    return (
      <div className="ed" data-testid="editor">
        <div className="ed-error" role="alert" data-testid="editor-error">{loadError}</div>
      </div>
    );
  }

  return (
    <div className="ed" data-testid="editor">
      <header className="ed-bar">
        <div className="ed-tools" role="toolbar" aria-label="Tools">
          {TOOL_GROUPS.map((g) => (
            <div key={g.label} className="ed-group" role="group" aria-label={g.label} data-label={g.label}>
              {g.tools.map((t) => (
                <button key={t.id} className="ed-tool" data-active={tool === t.id} aria-pressed={tool === t.id} aria-label={t.label} title={`${t.label}: ${t.hint}`} data-testid={`tool-${t.id}`} onClick={() => { setTool(t.id); setCrop(null); setTextAt(null); }}>
                  <t.icon />
                  <span className="ed-tool-label">{t.label}</span>
                </button>
              ))}
            </div>
          ))}
          <div className="ed-group" role="group" aria-label="Transform" data-label="Transform">
            <button className="ed-tool" data-testid="rotate-left" disabled={!doc} onClick={() => rotate(-1)} aria-label="Rotate left" title="Rotate 90° counter-clockwise"><RotateLeftIcon /><span className="ed-tool-label">Left</span></button>
            <button className="ed-tool" data-testid="rotate-right" disabled={!doc} onClick={() => rotate(1)} aria-label="Rotate right" title="Rotate 90° clockwise"><RotateRightIcon /><span className="ed-tool-label">Right</span></button>
            <button className="ed-tool" data-testid="resize-open" disabled={!doc} onClick={() => doc && setResizing({ w: doc.width, h: doc.height, lock: true })} aria-label="Resize" title="Change the image size"><ResizeIcon /><span className="ed-tool-label">Resize</span></button>
          </div>
        </div>
        <span className="grow" />
        <div className="ed-group" role="group" aria-label="History" data-label="History">
          <button className="ed-tool icon-only" data-testid="editor-undo" disabled={!hist || !canUndo(hist)} onClick={undoNow} aria-label="Undo" title="Undo (Ctrl+Z)"><UndoIcon /></button>
          <button className="ed-tool icon-only" data-testid="editor-redo" disabled={!hist || !canRedo(hist)} onClick={redoNow} aria-label="Redo" title="Redo (Ctrl+Y)"><RedoIcon /></button>
        </div>
        <div className="ed-output" role="group" aria-label="Output">
          <button className="ed-btn" data-testid="editor-copy" disabled={!doc || busy} onClick={() => void copy()} title="Copy the image to the clipboard"><CopyIcon />Copy</button>
          <button className="ed-btn" data-testid="editor-save-as" disabled={!doc || busy} onClick={() => void save(true)} title="Choose where to save (Ctrl+Shift+S)">Save as…</button>
          <button className="ed-btn primary" data-testid="editor-save" disabled={!doc || busy} onClick={() => void save(false)} title="Save to your screenshot folder (Ctrl+S)"><SaveIcon />Save</button>
        </div>
      </header>

      <div className="ed-opts" data-tool={tool}>
        {USES_COLOR.has(tool) && (
          <div className="ed-swatches" role="group" aria-label="Colour">
            {COLORS.map((c) => (
              <button key={c} className="ed-swatch" style={{ background: c }} data-active={color === c} aria-label={`Colour ${c}`} aria-pressed={color === c} data-testid={`color-${c.slice(1)}`} onClick={() => setColor(c)} />
            ))}
            <input type="color" aria-label="Custom colour" value={color} data-testid="editor-color" onChange={(e) => setColor(e.target.value)} />
          </div>
        )}
        {USES_WIDTH.has(tool) && (
          <label className="ed-field">
            Line
            <select aria-label="Line width" data-testid="editor-width" value={width} onChange={(e) => setWidth(Number(e.target.value))}>
              {[2, 4, 6, 10, 16].map((w) => <option key={w} value={w}>{w}</option>)}
            </select>
          </label>
        )}
        {tool === 'text' && (
          <label className="ed-field">
            Text
            <select aria-label="Text size" data-testid="editor-text-size" value={textSize} onChange={(e) => setTextSize(Number(e.target.value))}>
              {[18, 28, 40, 60, 90].map((w) => <option key={w} value={w}>{w}</option>)}
            </select>
          </label>
        )}
        <span className="ed-hint" data-testid="editor-hint">{toolInfo.hint}</span>
        <span className="grow" />
        {doc && <span className="ed-dims" data-testid="editor-size">{doc.width} × {doc.height}</span>}
      </div>

      {resizing && doc && (
        <div className="ed-panel" role="dialog" aria-label="Resize image" data-testid="resize-panel">
          <label className="ed-field">Width<input type="number" min={1} data-testid="resize-w" value={resizing.w} onChange={(e) => setResizing((r) => r && { ...r, ...resizeFrom({ w: doc.width, h: doc.height }, 'w', Number(e.target.value), r.lock, r) })} /></label>
          <label className="ed-field">Height<input type="number" min={1} data-testid="resize-h" value={resizing.h} onChange={(e) => setResizing((r) => r && { ...r, ...resizeFrom({ w: doc.width, h: doc.height }, 'h', Number(e.target.value), r.lock, r) })} /></label>
          <label className="ed-check"><input type="checkbox" data-testid="resize-lock" checked={resizing.lock} onChange={(e) => setResizing((r) => r && { ...r, lock: e.target.checked })} /> Keep proportions</label>
          {resizeError && <span className="ed-bad" data-testid="resize-error">{resizeError}</span>}
          <span className="grow" />
          <button className="ed-btn primary" data-testid="resize-apply" disabled={!!resizeError} onClick={applyResize}>Resize</button>
          <button className="ed-btn" data-testid="resize-cancel" onClick={() => setResizing(null)}>Cancel</button>
        </div>
      )}

      {crop && (
        <div className="ed-panel" data-testid="crop-panel">
          <span>Keep {crop.w} × {crop.h}</span>
          <span className="grow" />
          <button className="ed-btn primary" data-testid="crop-apply" onClick={applyCrop}>Apply crop</button>
          <button className="ed-btn" data-testid="crop-cancel" onClick={() => setCrop(null)}>Cancel</button>
        </div>
      )}

      <main className="ed-stage">
        {!doc ? (
          <div className="ed-loading" data-testid="editor-loading">Opening…</div>
        ) : (
          <canvas
            ref={canvasRef}
            className="ed-canvas"
            data-testid="editor-canvas"
            data-tool={tool}
            role="img"
            aria-label="Screenshot being edited"
            onPointerDown={onDown}
            onPointerMove={onMove}
            onPointerUp={onUp}
            onPointerCancel={onUp}
          />
        )}
        {textBox && (
          <input
            autoFocus
            className="ed-text"
            data-testid="text-input"
            aria-label="Text to add"
            style={{ left: textBox.left, top: textBox.top, fontSize: textBox.fontSize, color }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') finishText(e.currentTarget.value);
              if (e.key === 'Escape') setTextAt(null);
            }}
            onBlur={(e) => finishText(e.currentTarget.value)}
          />
        )}
      </main>

      <footer className="ed-status" data-testid="editor-status" data-kind={status?.kind ?? 'none'} role="status">
        {status ? (
          <>
            <span>{status.text}</span>
            {status.path && <button className="ed-link" data-testid="editor-reveal" onClick={() => void backend.screenshotReveal(status.path!).catch((e) => say(String(e instanceof Error ? e.message : e), 'error'))}>Show in folder</button>}
          </>
        ) : (
          <span className="ed-dim">Nothing leaves this PC unless you copy or save it.</span>
        )}
      </footer>
    </div>
  );
}

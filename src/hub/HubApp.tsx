import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { useBackend, useResolvedTheme, useSettings, useSubscription } from '../backend/hooks';
import { DEFAULT_ACCENT, onAccent } from '../island/accent';
import type { HubLayout, HubPlaced, WidgetDef } from '../shared/types';
import { ChevronLeftIcon, ChevronRightIcon, CheckIcon, CloseIcon, GridIcon, GripIcon, PinFilledIcon, PinOutlineIcon, PlusIcon, ResetIcon, SlidersIcon } from './icons';
import { addWidget, dropIndex, has, initialLayout, MAX_PINNED, moveWidget, pinnedCount, removeWidget, resizeWidget, setPinned, SIZE_LABEL, SPAN } from './layout';
import { entryOf } from './registry';
import { usePageActive, useHubLayout } from './useHub';
import { WidgetHost } from './WidgetHost';

interface Drag {
  kind: string;
  /** Pointer position, for the floating label. */
  x: number;
  y: number;
  /** Where the card would land if released now. */
  target: number;
  /** Card boxes when the drag began (the layout reflows while dragging, so they are frozen). */
  rects: DOMRect[];
}

export function HubApp() {
  const backend = useBackend();
  const { settings } = useSettings();
  const theme = useResolvedTheme(settings?.appearance.theme);
  const { catalog, layout, apply, saveError } = useHubLayout();
  const active = usePageActive();

  const [customise, setCustomise] = useState(false);
  const [gallery, setGallery] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);
  const [notice, setNotice] = useState('');
  const [announce, setAnnounce] = useState('');
  const [drag, setDrag] = useState<Drag | null>(null);
  const [flash, setFlash] = useState<string | null>(null);

  const cards = useRef(new Map<string, HTMLElement>());
  const dragRef = useRef<Drag | null>(null);
  dragRef.current = drag;
  const noticeTimer = useRef<number | undefined>(undefined);

  const a = settings?.appearance;
  const accent = a?.accentSource === 'custom' && a.customAccent ? a.customAccent : DEFAULT_ACCENT;

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const defs = useMemo(() => new Map(catalog.map((d) => [d.id, d])), [catalog]);

  const say = useCallback((text: string) => {
    setNotice(text);
    window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => setNotice(''), 3200);
  }, []);

  // ---- being asked to show one widget (island pin, ?focus=...)
  const showWidget = useCallback((kind: string) => {
    const el = cards.current.get(kind);
    if (!el) return;
    el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    setFlash(kind);
    window.setTimeout(() => setFlash((f) => (f === kind ? null : f)), 1200);
  }, []);
  const wanted = useRef(new URLSearchParams(location.search).get('focus'));
  useEffect(() => {
    if (layout && wanted.current && has(layout, wanted.current)) {
      const k = wanted.current;
      wanted.current = null;
      requestAnimationFrame(() => showWidget(k));
    }
  }, [layout, showWidget]);
  useSubscription<string>((cb) => backend.onHubFocus(cb), showWidget);

  // ---- operations
  const doAdd = (d: WidgetDef) => {
    apply((l) => addWidget(l, d));
    say(`${d.title} added.`);
  };
  const doRemove = (kind: string) => {
    const title = defs.get(kind)?.title ?? kind;
    apply((l) => removeWidget(l, kind));
    say(`${title} removed. You can add it again from Add widget; its data is kept.`);
  };
  const doPin = (kind: string, pinned: boolean) => {
    if (pinned && layout && pinnedCount(layout) >= MAX_PINNED) {
      say(`You can pin up to ${MAX_PINNED} widgets. Unpin one first.`);
      return;
    }
    apply((l) => setPinned(l, kind, pinned));
  };
  const doMove = (kind: string, to: number) => {
    apply((l) => moveWidget(l, kind, to));
    requestAnimationFrame(() => cards.current.get(kind)?.focus());
  };

  // ---- dragging by the handle. Pointer events on the window (not pointer capture): the card's DOM node moves
  // when the order changes mid-drag, which makes the browser drop a capture held by the handle. HTML5 drag and
  // drop is not used because the window's drop handler intercepts it on Windows.
  const layoutRef = useRef<HubLayout | null>(null);
  layoutRef.current = layout;
  const dragging = drag !== null;

  const startDrag = (e: ReactPointerEvent, kind: string) => {
    if (!layout || e.button !== 0) return;
    e.preventDefault();
    const rects = layout.widgets.map((w) => cards.current.get(w.kind)?.getBoundingClientRect() ?? new DOMRect());
    const from = layout.widgets.findIndex((w) => w.kind === kind);
    setDrag({ kind, x: e.clientX, y: e.clientY, target: from, rects });
  };

  useEffect(() => {
    if (!dragging) return;
    const move = (e: PointerEvent) => {
      const d = dragRef.current;
      if (d) setDrag({ ...d, x: e.clientX, y: e.clientY, target: dropIndex(d.rects, e.clientX, e.clientY) });
    };
    const end = (commit: boolean) => {
      const d = dragRef.current;
      if (!d) return;
      dragRef.current = null;
      setDrag(null);
      if (commit) {
        apply((l) => moveWidget(l, d.kind, d.target));
        const total = layoutRef.current?.widgets.length ?? 0;
        setAnnounce(`${defs.get(d.kind)?.title ?? d.kind} moved to position ${d.target + 1} of ${total}.`);
      }
    };
    const up = () => end(true);
    const cancel = () => end(false);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    return () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel);
    };
  }, [dragging, apply, defs]);

  const onCardKey = (e: ReactKeyboardEvent, p: HubPlaced, index: number) => {
    if (!customise || !e.altKey || e.target !== e.currentTarget) return;
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      doMove(p.kind, index - 1);
      setAnnounce(`${defs.get(p.kind)?.title ?? p.kind} moved to position ${Math.max(1, index)} of ${layout?.widgets.length}.`);
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      doMove(p.kind, index + 1);
      setAnnounce(`${defs.get(p.kind)?.title ?? p.kind} moved to position ${Math.min(layout?.widgets.length ?? 1, index + 2)} of ${layout?.widgets.length}.`);
    }
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (dragRef.current) {
        dragRef.current = null;
        setDrag(null);
      } else if (gallery) setGallery(false);
      else if (customise) setCustomise(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [gallery, customise]);

  const shown: HubLayout | null = useMemo(() => (layout && drag ? moveWidget(layout, drag.kind, drag.target) : layout), [layout, drag]);
  const available = catalog.filter((d) => !layout || !has(layout, d.id));
  const reduced = a?.motion === 'off';

  return (
    <div
      className="hub"
      data-testid="hub"
      data-customise={customise}
      data-dragging={!!drag}
      data-motion={a?.motion ?? 'normal'}
      data-reduced={reduced}
      style={{ '--accent': accent, '--on-accent': onAccent(accent) } as CSSProperties}
    >
      <header className="hub-head">
        <div className="hub-title">
          <GridIcon width={18} height={18} />
          <h1>Widgets</h1>
          {layout && <span className="hub-count" data-testid="hub-count">{layout.widgets.length}</span>}
        </div>
        <div className="hub-actions">
          {customise && (
            confirmReset ? (
              <span className="hub-confirm">
                Reset the layout?
                <button className="hub-btn danger" data-testid="hub-reset-yes" onClick={() => { apply(() => initialLayout(catalog)); setConfirmReset(false); say('Layout reset.'); }}>Reset</button>
                <button className="hub-btn" onClick={() => setConfirmReset(false)}>Cancel</button>
              </span>
            ) : (
              <button className="hub-btn" data-testid="hub-reset" onClick={() => setConfirmReset(true)}>
                <ResetIcon /> Reset layout
              </button>
            )
          )}
          <button className="hub-btn" data-testid="hub-add" aria-expanded={gallery} onClick={() => setGallery((g) => !g)}>
            <PlusIcon /> Add widget
          </button>
          <button className="hub-btn primary" data-testid="hub-customise" aria-pressed={customise} onClick={() => { setCustomise((c) => !c); setConfirmReset(false); }}>
            {customise ? <CheckIcon /> : <SlidersIcon />} {customise ? 'Done' : 'Customise'}
          </button>
        </div>
      </header>

      {(notice || saveError) && (
        <div className="hub-notice" role="status" data-testid="hub-notice" data-error={!!saveError}>
          {saveError ? `Could not save your layout: ${saveError}` : notice}
        </div>
      )}
      <div className="sr-only" aria-live="polite" data-testid="hub-announce">{announce}</div>

      <div className="hub-body">
        {gallery && (
          <aside className="hub-gallery" data-testid="hub-gallery" aria-label="Add a widget">
            <div className="gal-head">
              <span>Add a widget</span>
              <button className="icon-btn" aria-label="Close" data-testid="hub-gallery-close" onClick={() => setGallery(false)}><CloseIcon /></button>
            </div>
            {catalog.length === 0 && <div className="gal-empty">No widgets are available.</div>}
            {catalog.map((d) => {
              const placed = !!layout && has(layout, d.id);
              const Icon = entryOf(d.id)?.icon ?? GridIcon;
              return (
                <div className="gal-item" key={d.id} data-testid="gallery-item" data-kind={d.id} data-added={placed}>
                  <span className="gal-icon"><Icon width={18} height={18} /></span>
                  <div className="gal-text">
                    <div className="gal-title">{d.title}</div>
                    <div className="gal-desc">{d.description}</div>
                  </div>
                  <button className="hub-btn" disabled={placed} data-testid="gallery-add" aria-label={`Add ${d.title}`} onClick={() => doAdd(d)}>
                    {placed ? 'Added' : 'Add'}
                  </button>
                </div>
              );
            })}
            {catalog.length > 0 && available.length === 0 && <div className="gal-empty">Every available widget is already on your dashboard.</div>}
          </aside>
        )}

        <main className="hub-main">
          {shown && shown.widgets.length === 0 && (
            <div className="hub-empty" data-testid="hub-empty">
              <GridIcon width={30} height={30} />
              <div className="he-title">Your dashboard is empty</div>
              <div className="he-text">Add a widget to get started.</div>
              <button className="hub-btn primary" data-testid="hub-empty-add" onClick={() => setGallery(true)}><PlusIcon /> Add widget</button>
            </div>
          )}
          <div className="hub-grid" data-testid="hub-grid">
            {shown?.widgets.map((p, i) => {
              const def = defs.get(p.kind);
              const entry = entryOf(p.kind);
              const Icon = entry?.icon ?? GridIcon;
              const span = SPAN[p.size];
              const dragging = drag?.kind === p.kind;
              return (
                <section
                  key={p.kind}
                  ref={(el) => {
                    if (el) cards.current.set(p.kind, el);
                    else cards.current.delete(p.kind);
                  }}
                  className="card"
                  data-testid="widget-card"
                  data-kind={p.kind}
                  data-size={p.size}
                  data-pinned={p.pinned}
                  data-dragging={dragging}
                  data-flash={flash === p.kind}
                  tabIndex={customise ? 0 : -1}
                  aria-label={def?.title ?? p.kind}
                  style={{ gridColumn: `span ${span.cols}`, gridRow: `span ${span.rows}` }}
                  onKeyDown={(e) => onCardKey(e, p, i)}
                >
                  <header className="card-head">
                    {customise && (
                      <button
                        className="grip"
                        data-testid="card-grip"
                        aria-label={`Drag ${def?.title ?? p.kind}`}
                        title="Drag to reorder (or Alt + arrow keys on the card)"
                        onPointerDown={(e) => startDrag(e, p.kind)}
                      >
                        <GripIcon />
                      </button>
                    )}
                    <Icon width={15} height={15} />
                    <h2 className="card-title">{def?.title ?? p.kind}</h2>
                    <span className="spacer" />
                    {customise && (
                      <>
                        <button className="icon-btn" data-testid="card-earlier" aria-label={`Move ${def?.title ?? p.kind} earlier`} disabled={i === 0} onClick={() => { doMove(p.kind, i - 1); setAnnounce(`${def?.title ?? p.kind} moved to position ${i} of ${shown.widgets.length}.`); }}><ChevronLeftIcon /></button>
                        <button className="icon-btn" data-testid="card-later" aria-label={`Move ${def?.title ?? p.kind} later`} disabled={i === shown.widgets.length - 1} onClick={() => { doMove(p.kind, i + 1); setAnnounce(`${def?.title ?? p.kind} moved to position ${i + 2} of ${shown.widgets.length}.`); }}><ChevronRightIcon /></button>
                      </>
                    )}
                    <button
                      className="icon-btn"
                      data-testid="card-pin"
                      aria-pressed={p.pinned}
                      aria-label={p.pinned ? `Unpin ${def?.title ?? p.kind}` : `Pin ${def?.title ?? p.kind}`}
                      title={p.pinned ? 'Unpin from the island' : 'Pin to the island'}
                      onClick={() => doPin(p.kind, !p.pinned)}
                    >
                      {p.pinned ? <PinFilledIcon /> : <PinOutlineIcon />}
                    </button>
                    {customise && (
                      <button className="icon-btn danger" data-testid="card-remove" aria-label={`Remove ${def?.title ?? p.kind}`} title="Remove from the dashboard" onClick={() => doRemove(p.kind)}>
                        <CloseIcon />
                      </button>
                    )}
                  </header>
                  {customise && def && def.sizes.length > 1 && (
                    <div className="size-row" role="group" aria-label="Size">
                      {def.sizes.map((s) => (
                        <button key={s} className="chip" data-testid={`size-${s}`} aria-pressed={p.size === s} onClick={() => apply((l) => resizeWidget(l, p.kind, s, def))}>
                          {SIZE_LABEL[s]}
                        </button>
                      ))}
                    </div>
                  )}
                  <div className="card-body">
                    <WidgetHost kind={p.kind} size={p.size} active={active} onRemove={() => doRemove(p.kind)} />
                  </div>
                </section>
              );
            })}
          </div>
        </main>
      </div>

      {drag && (
        <div className="drag-label" data-testid="drag-label" style={{ transform: `translate(${drag.x + 12}px, ${drag.y + 12}px)` }}>
          {defs.get(drag.kind)?.title ?? drag.kind}
        </div>
      )}
    </div>
  );
}

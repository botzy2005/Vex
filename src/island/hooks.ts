import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import type { Backend } from '../backend/backend';
import { findActiveIndex } from '../lyrics/sync';
import { currentPositionMs } from '../lyrics/sync';
import type { LyricsDoc, MediaSession, Motion, Settings } from '../shared/types';
import { extractAccent } from './accent';
import type { Layout } from './layout';
import { MOTION_PRESETS, Spring } from './spring';

/** requestAnimationFrame loop that only runs while `active`. */
export function useFrameLoop(active: boolean, cb: (nowMs: number) => void) {
  const ref = useRef(cb);
  ref.current = cb;
  useEffect(() => {
    if (!active) return;
    let id = 0;
    const tick = (t: number) => {
      ref.current(t);
      id = requestAnimationFrame(tick);
    };
    id = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(id);
  }, [active]);
}

/** Stable function returning the live playback position (extrapolated between backend updates). */
export function usePlaybackPosition(session: MediaSession | null): () => number {
  const ref = useRef(session);
  ref.current = session;
  return useCallback(() => {
    const s = ref.current;
    return s ? currentPositionMs(s, Date.now()) : 0;
  }, []);
}

/** Index of the active lyric line for the live position. */
export function useActiveLine(
  doc: LyricsDoc | null,
  getPos: () => number,
  offsetMs: number,
  session: MediaSession | null,
): number {
  const [index, setIndex] = useState(-1);
  const running = !!doc && session?.status === 'playing';

  const evaluate = useCallback(() => {
    if (!doc) return setIndex(-1);
    setIndex((prev) => {
      const next = findActiveIndex(doc.lines, getPos() - offsetMs);
      return next === prev ? prev : next;
    });
  }, [doc, getPos, offsetMs]);

  // Paused / seeked / new track: evaluate once whenever the inputs change.
  useEffect(evaluate, [evaluate, session]);
  useFrameLoop(running, evaluate);
  return doc ? index : -1;
}

export function useAccent(settings: Settings | null, artworkUrl: string | undefined): string {
  const [fromArt, setFromArt] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    setFromArt(null);
    if (artworkUrl) extractAccent(artworkUrl).then((c) => !cancelled && setFromArt(c));
    return () => {
      cancelled = true;
    };
  }, [artworkUrl]);
  if (!settings) return '#0a84ff';
  return settings.appearance.accentSource === 'artwork' && fromArt ? fromArt : settings.appearance.customAccent;
}

export function useReducedMotion(): boolean {
  const [r, setR] = useState(() => window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  useEffect(() => {
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const on = (e: MediaQueryListEvent) => setR(e.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  return r;
}

/**
 * Animate the island's width / height / corner radii with springs, writing
 * straight to the element's style each frame (no React re-renders).
 */
export function useIslandSpring(ref: RefObject<HTMLElement | null>, layout: Layout, motion: Motion) {
  const springs = useRef<{ w: Spring; h: Spring; r: Spring; rt: Spring } | null>(null);
  const raf = useRef(0);
  const last = useRef(0);
  const cfgRef = useRef(MOTION_PRESETS.normal);
  const reduced = useReducedMotion();
  const instant = motion === 'off' || reduced;
  if (motion !== 'off') cfgRef.current = MOTION_PRESETS[motion];

  const apply = useCallback(() => {
    const el = ref.current;
    const s = springs.current;
    if (!el || !s) return;
    el.style.width = `${Math.max(0, s.w.x)}px`;
    el.style.height = `${Math.max(0, s.h.x)}px`;
    const rt = Math.max(0, s.rt.x);
    const r = Math.max(0, s.r.x);
    el.style.borderRadius = `${rt}px ${rt}px ${r}px ${r}px`;
  }, [ref]);

  // The island element mounts after settings load, i.e. after the effect below first ran;
  // re-apply the current spring values after every render so it never appears unsized.
  useLayoutEffect(() => {
    apply();
  });

  useLayoutEffect(() => {
    if (!springs.current) {
      springs.current = {
        w: new Spring(layout.width),
        h: new Spring(layout.height),
        r: new Spring(layout.radius),
        rt: new Spring(layout.radiusTop),
      };
    }
    const s = springs.current;
    s.w.target = layout.width;
    s.h.target = layout.height;
    s.r.target = layout.radius;
    s.rt.target = layout.radiusTop;

    cancelAnimationFrame(raf.current);
    if (instant) {
      Object.values(s).forEach((sp) => sp.snap());
      apply();
      return;
    }
    last.current = performance.now();
    const tick = (now: number) => {
      const dt = Math.min(0.05, (now - last.current) / 1000);
      last.current = now;
      const all = Object.values(s);
      all.forEach((sp) => sp.step(dt, cfgRef.current));
      apply();
      if (all.every((sp) => sp.atRest())) {
        all.forEach((sp) => sp.snap());
        apply();
        return;
      }
      raf.current = requestAnimationFrame(tick);
    };
    raf.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf.current);
  }, [layout.width, layout.height, layout.radius, layout.radiusTop, instant, apply]);
}

const SHRINK_DELAY_MS = 650;

/**
 * Keep the OS window just big enough for the island. Grow immediately (before
 * the spring starts) and shrink only after the collapse animation has had time
 * to finish, so the island is never clipped mid-animation.
 */
export function useWindowSize(backend: Backend, layout: Layout) {
  const applied = useRef({ w: 0, h: 0 });
  useEffect(() => {
    const t = { w: layout.windowWidth, h: layout.windowHeight };
    const a = applied.current;
    const m = { w: Math.max(t.w, a.w), h: Math.max(t.h, a.h) };
    if (m.w !== a.w || m.h !== a.h) {
      applied.current = m;
      void backend.setIslandSize(m.w, m.h);
    }
    if (m.w === t.w && m.h === t.h) return;
    const id = window.setTimeout(() => {
      applied.current = t;
      void backend.setIslandSize(t.w, t.h);
    }, SHRINK_DELAY_MS);
    return () => window.clearTimeout(id);
  }, [backend, layout.windowWidth, layout.windowHeight]);
}

export interface Expansion {
  expanded: boolean;
  pinned: boolean;
  onPointerEnter(): void;
  onPointerLeave(): void;
  expand(): void;
  collapse(): void;
  togglePinned(): void;
  /** Hotkey behaviour: expand and pin, or collapse and unpin. */
  toggleFromHotkey(): void;
}

/** Hover-to-expand with delays, click-to-expand, pinning, and a "keep open" lock (e.g. a focused input). */
/** `holdHover`: a confirmation with buttons is showing, so hovering must not expand the island out from under the pointer. */
export function useExpansion(island: Settings['island'] | undefined, locked: boolean, holdHover = false): Expansion {
  const [expanded, setExpanded] = useState(false);
  const [pinned, setPinned] = useState(false);
  const inside = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const cfg = useRef(island);
  cfg.current = island;
  const hold = useRef(holdHover);
  hold.current = holdHover;

  const clear = () => window.clearTimeout(timer.current);

  const scheduleCollapse = useCallback(() => {
    clear();
    timer.current = window.setTimeout(() => setExpanded(false), cfg.current?.collapseDelayMs ?? 350);
  }, []);

  // When the lock releases (input blurred) and the pointer is already away, collapse.
  useEffect(() => {
    if (!locked && expanded && !pinned && !inside.current) scheduleCollapse();
  }, [locked, expanded, pinned, scheduleCollapse]);

  useEffect(() => clear, []);

  return {
    expanded,
    pinned,
    onPointerEnter() {
      inside.current = true;
      clear();
      if (!expanded && cfg.current?.hoverToExpand && !hold.current) {
        timer.current = window.setTimeout(() => setExpanded(true), cfg.current?.hoverDelayMs ?? 120);
      }
    },
    onPointerLeave() {
      inside.current = false;
      clear();
      if (expanded && !pinned && !locked) scheduleCollapse();
    },
    expand() {
      clear();
      setExpanded(true);
    },
    collapse() {
      clear();
      setPinned(false);
      setExpanded(false);
    },
    togglePinned() {
      setPinned((p) => !p);
    },
    toggleFromHotkey() {
      clear();
      if (expanded) {
        setPinned(false);
        setExpanded(false);
      } else {
        setPinned(true);
        setExpanded(true);
      }
    },
  };
}

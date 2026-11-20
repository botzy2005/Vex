# VEX 2.0 design plan

Direction: macOS restraint (spacing, type, calm motion) + Windows Fluent (depth, layered surfaces) + VEX's own dark, indigo-accented look.

## Findings (Phase 0)
- One island window (`src/island/App.tsx`) renders compact views and an expanded tabbed panel; size is animated by a JS spring (`useIslandSpring`), content cross-fades with CSS (`view-in`).
- Three separate hand-written palettes (island `.stage`, settings `:root`, editor `:root`), with hard-coded durations, radii and button styles repeated per module. The island accent default was iOS blue.
- The window is transparent with no native acrylic, so a real backdrop blur is not available. `blur()` filters were used inside entrance animations (costly on integrated GPUs) and a decorative gradient drifted continuously while expanded.
- Accent text was hard-coded white, which fails contrast on light accents such as the new default.
- Many personalisation settings already exist (theme, accent, size, corner radius, glass opacity, animation intensity, monitor, offset, lyrics speed). None is added without a real effect; "blur intensity" is deliberately not offered because it cannot be implemented honestly in a transparent window.

## Stages (each its own commit, CI-verified)
1. Shared tokens (`src/styles/tokens.css`): palette, spacing, radii, type, control sizes, durations, easings, shadows; island, settings and editor consume them. Accent contrast helper.
2. Island surface, compact and expanded polish.
3. Animation system: token timings, transform/opacity only, press feedback, reduced motion, no idle animation.
4. Media and lyrics.
5. Assistant.
6. Screenshot and clipboard.
7. Settings redesign.
8. Windows/responsive polish and final regression.

Not touched: Rust capture/clipboard/media/AI logic, provider and Ollama settings, credentials, installer config.

# Third-party notices and provenance

## Origin of the code
Everything in this repository was written for Islet. **No source code, assets, icons, names or
copy were taken from Notchify, Fenn/Nook, Oyenza Studio, QNotch, V-Notch, DynamicWin or any other
reference project.** Those projects were studied only through their public descriptions to decide
which features a Windows "Dynamic Island" app commonly offers.

- Notchify is closed source; only its publicly described behaviour informed the feature list.
- DynamicWin is CC BY-SA 4.0 and was deliberately not consulted for code.
- The island visuals (rounded black glass pill, spring expansion, tabbed panel) are the general
  Dynamic Island idiom. The icons are drawn for this project, the app icon is generated, and the
  assistant face is an original design.

## Dependencies (all permissively licensed; resolved versions are in Cargo.lock / package-lock.json)
| Component | License |
|---|---|
| Tauri 2 and tauri-plugin-global-shortcut | MIT OR Apache-2.0 |
| React 19 | MIT |
| Vite, Vitest | MIT |
| serde, serde_json, thiserror, tempfile | MIT OR Apache-2.0 |

Run `cargo license` / `npx license-checker` before a public release to regenerate the full list.

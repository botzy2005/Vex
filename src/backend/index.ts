import type { Backend } from './backend';

/**
 * Inside the VEX app the Tauri backend is used. The in-browser test backend
 * exists only for `npm run dev` and automated UI tests; the bundler removes it
 * from production builds (import.meta.env.DEV is statically false there).
 */
export async function createBackend(): Promise<Backend> {
  if ('__TAURI_INTERNALS__' in window) {
    const { createTauriBackend } = await import('./tauri');
    return createTauriBackend();
  }
  if (import.meta.env.DEV) {
    const { createTestBackend } = await import('./test');
    return createTestBackend();
  }
  throw new Error('VEX must be run through its desktop shell (the Tauri runtime was not found).');
}

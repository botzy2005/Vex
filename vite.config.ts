import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// Pages: the island overlay (index.html), the settings window (settings.html), the screenshot editor
// (editor.html) the region-selection overlay (overlay.html) and the Quick Launcher (launcher.html).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: 'chrome105', // WebView2 baseline
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, 'index.html'),
        settings: resolve(import.meta.dirname, 'settings.html'),
        editor: resolve(import.meta.dirname, 'editor.html'),
        overlay: resolve(import.meta.dirname, 'overlay.html'),
        launcher: resolve(import.meta.dirname, 'launcher.html'),
      },
    },
  },
  test: { include: ['tests/**/*.test.ts'], environment: 'node' },
});

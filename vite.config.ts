import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// Two pages: the island overlay (index.html) and the settings window (settings.html).
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
      },
    },
  },
  test: { include: ['tests/**/*.test.ts'], environment: 'node' },
});

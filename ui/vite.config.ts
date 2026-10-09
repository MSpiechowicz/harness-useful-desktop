import { fileURLToPath } from 'node:url';

import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// `ui/` is the Vite root regardless of the directory the CLI is started from.
const uiRoot = fileURLToPath(new URL('.', import.meta.url));
const debug = !!process.env.TAURI_ENV_DEBUG;
const libDir = fileURLToPath(new URL('./src/lib', import.meta.url));

export default defineConfig({
  root: uiRoot,
  plugins: [svelte(), tailwindcss()],
  resolve: { alias: { $lib: libDir } },
  // Keep Rust compiler output visible in the terminal during `tauri dev`.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || false,
    watch: { ignored: ['**/src-tauri/**', '**/business/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    outDir: fileURLToPath(new URL('./dist', import.meta.url)),
    emptyOutDir: true,
    // The production CSP is `font-src 'self'`: never inline fonts or other assets as data: URIs.
    assetsInlineLimit: 0,
    // ES2022 floor: Svelte 5 output and the WebViews Tauri ships against
    // (WebView2, WebKitGTK 4.1, macOS 12.3+ WKWebView) all support it, so no
    // down-leveling is needed. Tauri's template default (safari13) would only add bundle size.
    target: 'es2022',
    // Vite 8 minifies with oxc; esbuild is no longer bundled.
    minify: debug ? false : 'oxc',
    sourcemap: debug,
  },
});

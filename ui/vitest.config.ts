import { fileURLToPath } from 'node:url';

import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig } from 'vitest/config';

// Tests run from the repository root so globs and CI `--shard` stay stable.
const repoRoot = fileURLToPath(new URL('..', import.meta.url));
const libDir = fileURLToPath(new URL('./src/lib', import.meta.url));

export default defineConfig({
  root: repoRoot,
  resolve: { alias: { $lib: libDir } },
  plugins: [svelte({ configFile: fileURLToPath(new URL('./svelte.config.js', import.meta.url)) }), svelteTesting()],
  test: {
    environment: 'happy-dom',
    setupFiles: [fileURLToPath(new URL('./tests/setup.ts', import.meta.url))],
    include: ['ui/src/**/*.test.ts', 'ui/tests/**/*.test.ts'],
  },
});

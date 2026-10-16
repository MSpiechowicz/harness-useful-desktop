// `bun run dev:second [port]`: a second dev instance beside a running `bun run dev`.
// Differs from the first in the three things that make two instances collide: the Vite port, the bundle identifier
// (the single-instance lock and the app's config/data folders are keyed by it, so this one also keeps its own
// preferences), and the product name (window and tray title).
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const port = Number(process.argv[2] ?? process.env.HARNESS_DEV_PORT ?? 1421);

if (!Number.isInteger(port) || port < 1024 || port > 65535 || port === 1420) {
  console.error(`dev:second needs a free port other than 1420, got "${process.argv[2] ?? port}"`);
  process.exit(1);
}

const conf = await Bun.file(`${root}src-tauri/tauri.conf.json`).json();
const devUrl = `http://localhost:${port}`;

const override = {
  productName: `${conf.productName} (dev ${port})`,
  identifier: `${conf.identifier}.dev${port}`,
  build: {
    devUrl,
    beforeDevCommand: `bun run dev:ui --port ${port}`,
  },
  app: {
    security: { devCsp: conf.app.security.devCsp.replace('ws://localhost:1420', `ws://localhost:${port}`) },
  },
};

const child = Bun.spawn(['bun', 'run', 'tauri', 'dev', '--config', JSON.stringify(override)], {
  cwd: root,
  stdio: ['inherit', 'inherit', 'inherit'],
});

for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.on(signal, () => child.kill(signal));
}

process.exit(await child.exited);

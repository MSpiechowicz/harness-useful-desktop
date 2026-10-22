// Starts `tauri dev` for `bun run dev` and `dev:second`, forwards Ctrl+C, and exits with its code.
//
// Linux: WebKitGTK 2.54's DMA-BUF renderer makes the window blink on view switches and hovers under KWin 6.8
// on Wayland (issue #40). The release AppImage bundles an older WebKitGTK and does not blink. A value the user
// already set wins, so `WEBKIT_DISABLE_DMABUF_RENDERER=0 bun run dev` still tries the DMA-BUF renderer.
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));

export function devEnv(): Record<string, string | undefined> {
  const env: Record<string, string | undefined> = { ...process.env };
  if (process.platform === 'linux') {
    env.WEBKIT_DISABLE_DMABUF_RENDERER ??= '1';
  }
  return env;
}

export async function runTauriDev(args: string[]): Promise<never> {
  const child = Bun.spawn(['bun', 'run', 'tauri', 'dev', ...args], {
    cwd: root,
    env: devEnv(),
    stdio: ['inherit', 'inherit', 'inherit'],
  });

  for (const signal of ['SIGINT', 'SIGTERM'] as const) {
    process.on(signal, () => child.kill(signal));
  }

  process.exit(await child.exited);
}

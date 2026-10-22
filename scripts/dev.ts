// `bun run dev`: `tauri dev` with the dev environment (see `tauri-dev.ts`). Extra arguments go to `tauri dev`.
import { runTauriDev } from './tauri-dev';

await runTauriDev(process.argv.slice(2));

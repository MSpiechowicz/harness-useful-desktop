// Builds the `harness-memory` sidecar (crate `memory/`) and stages it where Tauri's `bundle.externalBin` expects it.
//
//   bun run sidecar             # debug build for the host triple (what `bun run dev` needs)
//   bun run sidecar --release   # release build for the host triple
//
// Tauri runs this as its before-dev/before-build command and sets TAURI_ENV_TARGET_TRIPLE (the `--target` of
// `tauri build`) and TAURI_ENV_DEBUG ("true" for `tauri dev` and `tauri build --debug`, "false" otherwise), so
// the staged binary matches the app's target and profile. Run by hand, neither is set: the host triple and a
// debug build apply unless `--release` is given.
//
// Output: src-tauri/binaries/harness-memory-<triple>[.exe]. tauri-build copies it next to the app executable as
// `harness-memory[.exe]` (the bundles ship it there), and the app installs it to a stable path from that copy.
// `cargo build/clippy/test -p harness_desktop` fails while that file is missing, so run this first.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

const NAME = 'harness-memory';
const ROOT = resolve(import.meta.dir, '..');

function fail(message: string): never {
  console.error(`build-sidecar: ${message}`);
  process.exit(1);
}

/** Runs `argv` (never through a shell) and returns its stdout; a non-zero exit stops the script. */
function run(argv: string[], inherit: boolean): string {
  const [command, ...args] = argv;
  const result = spawnSync(command, args, {
    cwd: ROOT,
    encoding: 'utf8',
    stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'inherit'],
  });

  if (result.error) {
    fail(`cannot run ${command}: ${result.error.message}`);
  }

  if (result.status !== 0) {
    fail(`${argv.join(' ')} exited with ${result.status}`);
  }

  return result.stdout ?? '';
}

function hostTriple(): string {
  const host = run(['rustc', '-vV'], false)
    .split('\n')
    .find((line) => line.startsWith('host: '));

  if (!host) {
    fail('`rustc -vV` reports no host triple');
  }

  return host.slice('host: '.length).trim();
}

const explicitTriple = process.env.TAURI_ENV_TARGET_TRIPLE;
const triple = explicitTriple || hostTriple();
const release = process.argv.includes('--release') || process.env.TAURI_ENV_DEBUG === 'false';
const profile = release ? 'release' : 'debug';
const extension = triple.includes('windows') ? '.exe' : '';

// Only an explicit target puts the output under target/<triple>/; that is also where `tauri build --target`
// leaves the app, and where tauri-build copies the sidecar from into.
const cargoArgs = ['cargo', 'build', '-p', 'harness_memory', '--bin', NAME];

if (release) {
  cargoArgs.push('--release');
}

if (explicitTriple) {
  cargoArgs.push('--target', explicitTriple);
}

run(cargoArgs, true);

const built = join(ROOT, 'target', ...(explicitTriple ? [explicitTriple] : []), profile, `${NAME}${extension}`);
const staged = join(ROOT, 'src-tauri', 'binaries', `${NAME}-${triple}${extension}`);

mkdirSync(join(ROOT, 'src-tauri', 'binaries'), { recursive: true });
copyFileSync(built, staged);
console.log(`build-sidecar: ${profile} ${triple} -> ${staged}`);

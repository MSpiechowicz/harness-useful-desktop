// Guard: Tauri commands registered in Rust must match the TypeScript command map 1:1.
//
// Rust side: every function annotated `#[tauri::command]` under `src-tauri/src/**/*.rs`.
// TS side:   top-level keys of `export const commands = { ... }` in `ui/src/lib/ipc/commands.ts`.
//
// Expected TS format: the map starts at a line beginning `export const commands` and ends at the
// first following line that starts with `}` in column 0. Each entry starts a line with exactly two
// spaces of indentation followed by the snake_case command name and a colon, e.g.
//   `  project_pick: cmd<ProjectPickArgs, ProjectPickResult>(),`
//   `  agent_stop: {`
// Deeper-indented lines and comments are ignored.
//
// A name present on only one side fails the guard. With no commands on either side it passes.
import { existsSync } from 'node:fs';
import { join } from 'node:path';

import { failWith, listFiles, readText, repoRoot } from './git-files';

const COMMANDS_TS = 'ui/src/lib/ipc/commands.ts';

// `#[tauri::command]` (optionally with arguments), other attributes/comments, then the fn name.
const RUST_COMMAND =
  /#\[tauri::command(?:\([^\]]*\))?\]\s*(?:(?:#\[[^\]]*\]|\/\/[^\n]*)\s*)*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)/g;

function rustCommandNames(): Set<string> {
  const names = new Set<string>();

  for (const path of listFiles()) {
    if (!path.startsWith('src-tauri/src/') || !path.endsWith('.rs')) {
      continue;
    }

    for (const match of readText(path).matchAll(RUST_COMMAND)) {
      names.add(match[1]);
    }
  }

  return names;
}

function tsCommandNames(): Set<string> {
  const names = new Set<string>();
  if (!existsSync(join(repoRoot, COMMANDS_TS))) {
    return names;
  }

  const lines = readText(COMMANDS_TS).split(/\r?\n/);
  const start = lines.findIndex((line) => /^export const commands\b/.test(line));
  if (start === -1) {
    failWith('check-ipc-contract: cannot find the command map', [
      `${COMMANDS_TS} must contain a line starting with "export const commands"`,
    ]);
  }

  for (const line of lines.slice(start + 1)) {
    if (line.startsWith('}')) {
      break;
    }

    const match = /^ {2}([a-z][a-z0-9_]*)\s*:/.exec(line);
    if (match) {
      names.add(match[1]);
    }
  }

  return names;
}

function difference(left: Set<string>, right: Set<string>): string[] {
  return [...left].filter((name) => !right.has(name)).sort();
}

const rust = rustCommandNames();
const ts = tsCommandNames();

if (rust.size === 0 && ts.size === 0) {
  console.log('check-ipc-contract: no IPC commands yet');
  process.exit(0);
}

const problems = [
  ...difference(rust, ts).map((name) => `${name}: Rust #[tauri::command] has no entry in ${COMMANDS_TS}`),
  ...difference(ts, rust).map((name) => `${name}: listed in ${COMMANDS_TS} but no Rust #[tauri::command] found`),
];

if (problems.length > 0) {
  failWith(`check-ipc-contract: ${problems.length} mismatch(es)`, problems);
}

console.log(`check-ipc-contract: ok (${rust.size} commands)`);

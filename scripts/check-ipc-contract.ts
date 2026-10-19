// Guard: Tauri commands registered in Rust must match the TypeScript command map 1:1.
//
// Rust side: every function annotated `#[tauri::command]` under `src-tauri/src/**/*.rs`.
// TS side:   top-level keys of `export const commands = { ... }` in `ui/src/lib/ipc/commands.ts`.
//
// Expected TS format: the map starts at a line beginning `export const commands` and ends at the
// first following line that starts with `}` in column 0. A `  ...name,` line spreads the map of another
// file (see SPREAD_FILES), which starts at `export const name = {`. Each entry starts a line with exactly two
// spaces of indentation followed by the snake_case command name and a colon, e.g.
//   `  project_pick: cmd<ProjectPickArgs, ProjectPickResult>(),`
//   `  agent_stop: {`
// Deeper-indented lines and comments are ignored.
//
// A name present on only one side fails the guard. With no commands on either side it passes.
//
// Tauri also rejects a command at runtime ("not allowed by ACL") unless it is listed in `COMMANDS` in
// `src-tauri/build.rs` and granted as `allow-<kebab-name>` in `src-tauri/capabilities/main.json`, so every
// Rust command must appear in both, and neither may list a command that does not exist.
import { existsSync } from 'node:fs';
import { join } from 'node:path';

import { failWith, listFiles, readText, repoRoot } from './git-files';

const COMMANDS_TS = 'ui/src/lib/ipc/commands.ts';
// Object spread into the map (`...memoryCommands,`): its file holds more entries in the same format.
const SPREAD_FILES: Record<string, string> = { memoryCommands: 'ui/src/lib/ipc/commands-memory.ts' };
const BUILD_RS = 'src-tauri/build.rs';
const CAPABILITY = 'src-tauri/capabilities/main.json';

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

/** The entry names of the map that starts at the line matching `start`, up to the first `}` in column 0. */
function mapNames(path: string, start: RegExp): Set<string> {
  const names = new Set<string>();
  const lines = readText(path).split(/\r?\n/);
  const first = lines.findIndex((line) => start.test(line));
  if (first === -1) {
    failWith('check-ipc-contract: cannot find the command map', [`${path} must contain a line matching ${start}`]);
  }

  for (const line of lines.slice(first + 1)) {
    if (line.startsWith('}')) {
      break;
    }

    const spread = /^ {2}\.\.\.(\w+),/.exec(line);
    const spreadPath = spread ? SPREAD_FILES[spread[1]] : undefined;
    if (spread && !spreadPath) {
      failWith('check-ipc-contract: unknown spread in the command map', [`${spread[1]} is not listed in SPREAD_FILES`]);
    }
    if (spread && spreadPath) {
      mapNames(spreadPath, new RegExp(`^export const ${spread[1]}\\b`)).forEach((name) => names.add(name));
    }

    const match = /^ {2}([a-z][a-z0-9_]*)\s*:/.exec(line);
    if (match) {
      names.add(match[1]);
    }
  }

  return names;
}

function tsCommandNames(): Set<string> {
  if (!existsSync(join(repoRoot, COMMANDS_TS))) {
    return new Set();
  }

  return mapNames(COMMANDS_TS, /^export const commands\b/);
}

/** The names in `const COMMANDS: &[&str] = &[ ... ];` of the build script. */
function aclCommandNames(): Set<string> {
  const list = /const COMMANDS: &\[&str\] = &\[([^\]]*)\];/.exec(readText(BUILD_RS));
  if (!list) {
    failWith('check-ipc-contract: cannot find the ACL command list', [`${BUILD_RS} must define const COMMANDS: &[&str] = &[ ... ];`]);
  }

  return new Set([...list[1].matchAll(/"([a-z][a-z0-9_]*)"/g)].map((match) => match[1]));
}

/** App commands granted by the main capability, back in snake_case (plugin and core grants contain a colon). */
function grantedCommandNames(): Set<string> {
  const capability = JSON.parse(readText(CAPABILITY)) as { permissions?: unknown[] };

  return new Set(
    (capability.permissions ?? [])
      .filter((permission): permission is string => typeof permission === 'string' && permission.startsWith('allow-'))
      .map((permission) => permission.slice('allow-'.length).replaceAll('-', '_')),
  );
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

const acl = aclCommandNames();
const granted = grantedCommandNames();

const problems = [
  ...difference(rust, ts).map((name) => `${name}: Rust #[tauri::command] has no entry in ${COMMANDS_TS}`),
  ...difference(ts, rust).map((name) => `${name}: listed in ${COMMANDS_TS} but no Rust #[tauri::command] found`),
  ...difference(rust, acl).map((name) => `${name}: missing from COMMANDS in ${BUILD_RS} (Tauri rejects it as "not allowed by ACL")`),
  ...difference(acl, rust).map((name) => `${name}: listed in COMMANDS in ${BUILD_RS} but no Rust #[tauri::command] found`),
  ...difference(rust, granted).map((name) => `${name}: not granted in ${CAPABILITY} (add "allow-${name.replaceAll('_', '-')}")`),
  ...difference(granted, rust).map((name) => `${name}: granted in ${CAPABILITY} but no Rust #[tauri::command] found`),
];

if (problems.length > 0) {
  failWith(`check-ipc-contract: ${problems.length} mismatch(es)`, problems);
}

console.log(`check-ipc-contract: ok (${rust.size} commands)`);

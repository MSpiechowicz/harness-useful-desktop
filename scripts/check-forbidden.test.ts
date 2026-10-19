// Run with `bun test scripts/`. Runs a copy of the guard on a scratch git repo holding one sample file,
// so the real sources are never touched. Samples that name a banned flag are assembled at run time:
// this file is itself scanned by the real guard.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const repoRoot = resolve(import.meta.dir, '..');

const ENGINE = 'business/src/opencode_server/sample.rs';
const SESSION = 'business/src/agent/opencode/sample.rs';
const HARNESS = 'business/src/harnesses/opencode/sample.rs';
const ROUTES = 'business/src/opencode_server/routes.rs';

let scratch: string;

beforeEach(() => {
  scratch = mkdtempSync(join(tmpdir(), 'guard-'));
  mkdirSync(join(scratch, 'scripts'));
  for (const name of ['check-forbidden.ts', 'git-files.ts']) {
    cpSync(join(repoRoot, 'scripts', name), join(scratch, 'scripts', name));
  }
  expect(Bun.spawnSync(['git', 'init', '-q'], { cwd: scratch }).success).toBe(true);
});

afterEach(() => {
  rmSync(scratch, { recursive: true, force: true });
});

/** Runs the guard on `source` saved as `path`: the exit code and the violation lines. */
function scan(path: string, source: string): { ok: boolean; output: string } {
  const file = join(scratch, path);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, source);

  const result = Bun.spawnSync(['bun', 'scripts/check-forbidden.ts'], { cwd: scratch });

  return { ok: result.success, output: `${result.stdout}${result.stderr}` };
}

function expectViolation(path: string, source: string, id: string) {
  const { ok, output } = scan(path, source);

  expect(ok).toBe(false);
  expect(output).toContain(`[${id}]`);
}

function expectClean(path: string, source: string) {
  const { ok, output } = scan(path, source);

  expect(output).toContain('check-forbidden: ok');
  expect(ok).toBe(true);
}

const route = (path: string) => `fn target() -> String { format!("${path}") }\n`;
const argv = (...tokens: string[]) => `const ARGS: [&str; ${tokens.length}] = [${tokens.map((t) => `"${t}"`).join(', ')}];\n`;

describe('OpenCode route and flag rules', () => {
  test.each([
    ['opencode-share', '/session/{session}/share'],
    ['opencode-share', '/session/{session}/unshare'],
    ['opencode-global-admin', '/global/dispose'],
    ['opencode-global-admin', '/global/upgrade'],
    ['opencode-global-admin', '/global/config'],
    ['opencode-auth', '/auth/{provider}'],
    ['opencode-auth', '/provider/{id}/oauth/authorize'],
    ['opencode-auth', '/provider'],
    ['opencode-surface', '/experimental/session'],
    ['opencode-surface', '/tui/append-prompt'],
    ['opencode-surface', '/pty/{id}'],
    ['opencode-config-write', '/config/{key}'],
  ])('%s forbids the route %s in the server client', (id, path) => {
    expectViolation(ENGINE, route(path), id);
  });

  test.each([ENGINE, SESSION, HARNESS])('the route rules cover %s', (path) => {
    expectViolation(path, route('/global/dispose'), 'opencode-global-admin');
  });

  test('a Config route with a write method is forbidden', () => {
    const patch = 'fn m(&self) -> Method { match self { Self::Config { .. } => Method::Patch, _ => Method::Get } }\n';

    expectViolation(ROUTES, patch, 'opencode-config-write');
  });

  test('a Config route inside an or-pattern with a write method is forbidden', () => {
    const post = 'fn m(&self) -> Method { match self { Self::Health | Self::Config { .. } | Self::Agents { .. } => Method::Post } }\n';

    expectViolation(ROUTES, post, 'opencode-config-write');
  });

  test.each([
    ['--mdns'],
    ['--mdns=true'],
  ])('opencode-mdns forbids %s', (flag) => {
    expectViolation(HARNESS, argv('serve', flag), 'opencode-mdns');
  });

  test('opencode-port forbids pinning port 4096', () => {
    expectViolation(HARNESS, argv('serve', '--port', '4096'), 'opencode-port');
    expectViolation(HARNESS, argv('serve', '--port=4096'), 'opencode-port');
  });

  test('the real read routes, flags and comments pass', () => {
    const allowed = [
      '//! Left out on purpose: `/share`, `/global/dispose`, `/auth/*`, "oauth", `/tui/*`.',
      route('/config'),
      route('/config?directory={directory}'),
      route('/config/providers'),
      route('/session/{session}/prompt_async'),
      'let data = "/home/u/.local/share/opencode";\n',
      argv('serve', '--hostname', '127.0.0.1', '--port', '0', '--mdns=false'),
      'fn m(&self) -> Method { match self { Self::Health | Self::Config { .. } | Self::ConfigProviders { .. } => Method::Get, _ => Method::Post } }\n',
    ].join('');

    expectClean(ROUTES, allowed);
  });

  test('the OpenCode rules stay out of the rest of the code', () => {
    expectClean('business/src/other/sample.rs', route('/session/{session}/share'));
  });
});

describe('rules that already cover OpenCode files', () => {
  const flag = (name: string) => ['--', name].join('');

  test.each(['auto', 'yolo', 'dangerously-skip-permissions'])('--%s is forbidden in the engine', (name) => {
    expectViolation(SESSION, argv('serve', flag(name)), name === 'auto' ? 'auto-flag' : `${name === 'yolo' ? 'yolo' : 'skip-permissions'}-flag`);
  });

  test('login, mcp and private-store argv are forbidden in the harness wiring', () => {
    const login = ['log', 'in'].join('');
    const mcp = ['m', 'cp'].join('');

    expectViolation(HARNESS, argv('auth', login), 'login-argv');
    expectViolation(HARNESS, argv('providers', login), 'login-argv');
    expectViolation(HARNESS, argv(mcp, 'auth'), 'mcp-argv');
    expectViolation(HARNESS, `let args = vec![${['"db"', '"path"'].join(', ')}];\n`, 'private-store-argv');
    expectViolation(HARNESS, `let args = vec![${['"account"'].join(', ')}];\n`, 'private-store-argv');
  });
});

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
const ENGINE_V2 = 'business/src/opencode_server_v2/sample.rs';
const SESSION_V2 = 'business/src/agent/opencode_v2/sample.rs';
const ROUTES_V2 = 'business/src/opencode_server_v2/routes.rs';

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

  test.each([ENGINE, SESSION, HARNESS, ENGINE_V2, SESSION_V2])('the route rules cover %s', (path) => {
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
    ['opencode-share', '/api/session/{session}/share'],
    ['opencode-auth', '/api/provider'],
    ['opencode-auth', '/api/provider/{id}/oauth/authorize'],
    ['opencode-surface', '/api/experimental/x'],
    ['opencode-surface', '/api/experimental/config'],
    ['opencode-surface', '/api/pty/{id}'],
    ['opencode-config-write', '/api/config/{key}'],
  ])('%s forbids the OpenCode 2 route %s', (id, path) => {
    expectViolation(ENGINE_V2, route(path), id);
  });

  test('an OpenCode 2 Config route with a write method is forbidden', () => {
    const patch =
      'fn m(&self) -> Method { match self { Self::Info | Self::ConfigGet { .. } | Self::ModelList { .. } => Method::Patch, _ => Method::Get } }\n';

    expectViolation(ROUTES_V2, patch, 'opencode-config-write');
    expectViolation(ROUTES_V2, 'fn m(r: RouteV2) -> Method { match r { RouteV2::ConfigSet { .. } => Method::Post } }\n', 'opencode-config-write');
  });

  test('the OpenCode 2 read routes pass', () => {
    const allowed = [
      '//! Left out on purpose: `/api/experimental/config`, share, PTYs.\n',
      route('/api/config?location%5Bdirectory%5D={directory}'),
      route('/api/session/{session}/prompt'),
      route('/api/shell/{shell}/output'),
      route('/api/permission/request'),
      argv('api', '--standalone', 'GET', '/api/config'),
      'fn m(&self) -> Method { match self { Self::Info | Self::ConfigGet { .. } | Self::ModelList { .. } => Method::Get, _ => Method::Post } }\n',
    ].join('');

    expectClean(ROUTES_V2, allowed);
  });

  test.each([
    ['opencode-service', ['--', 'service'].join('')],
    ['opencode-service', ['--', 'stdio'].join('')],
    ['opencode-service', 'service'],
    ['opencode-cors', ['--', 'cors'].join('')],
    ['opencode-cors', `${['--', 'cors'].join('')}=*`],
  ])('%s forbids %s', (id, flag) => {
    expectViolation(HARNESS, argv('serve', flag), id);
    expectViolation(SESSION_V2, argv('serve', flag), id);
  });

  test('a comment that names the service or cors flags passes, the same token in code does not', () => {
    const service = ['--', 'service'].join('');
    const cors = ['--', 'cors'].join('');
    const comments = [
      `//! Never \`${service}\`, never "service": the chat runs its own loopback server.\n`,
      `    /// No \`${cors}\`, so a browser origin cannot call it.\n`,
      `// "${cors}=*" stays out.\n`,
    ].join('');

    expectClean(SESSION_V2, comments);
    expectClean(ENGINE_V2, comments);
    expectViolation(SESSION_V2, `${comments}let flag = "${service}";\n`, 'opencode-service');
    expectViolation(ENGINE_V2, `${comments}let flag = "${cors}";\n`, 'opencode-cors');
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
    const id = name === 'auto' ? 'auto-flag' : `${name === 'yolo' ? 'yolo' : 'skip-permissions'}-flag`;

    expectViolation(SESSION, argv('serve', flag(name)), id);
    expectViolation(SESSION_V2, argv('serve', flag(name)), id);
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

describe('Claude Code permission-mode rules', () => {
  const CLAUDE_LAUNCH = 'business/src/harnesses/claude/sample.rs';
  const CLAUDE_SESSION = 'business/src/agent/claude/sample.rs';
  const flag = ['--permission', 'mode'].join('-');

  test.each(['bypassPermissions', 'auto', 'dontAsk', 'acceptEdits', 'plan'])(
    'starting in the %s mode is forbidden in the engine',
    (mode) => {
      expectViolation(CLAUDE_LAUNCH, argv(flag, mode), 'claude-permission-mode');
      expectViolation(CLAUDE_SESSION, `let arg = "${flag}=${mode}";\n`, 'claude-permission-mode');
      expectViolation(CLAUDE_SESSION, `//! runs \`${flag}  ${mode}\`\n`, 'claude-permission-mode');
    },
  );

  test('switching the mode over the control protocol is forbidden in the engine', () => {
    const subtype = ['set', 'permission', 'mode'].join('_');

    expectViolation('business/src/claude_stream/sample.rs', `fn s() -> &'static str { "${subtype}" }\n`, 'claude-permission-mode');
  });

  test('the default mode passes', () => {
    const allowed = [
      argv(flag, 'default'),
      `let arg = "${flag}=default";\n`,
      `fn a() -> Vec<String> { flag_with("${flag}", WIRE_DEFAULT) }\n`,
      `//! starts in \`${flag} default\`\n`,
    ].join('');

    expectClean(CLAUDE_LAUNCH, allowed);
  });

  test('the rules stay out of the rest of the code', () => {
    expectClean('business/src/other/sample.rs', argv(flag, 'plan'));
  });
});

describe('Claude Code settings and prompt-tool rules', () => {
  const CLAUDE_LAUNCH = 'business/src/harnesses/claude/sample.rs';
  const CLAUDE_SESSION = 'business/src/agent/claude/sample.rs';
  const flag = (...parts: string[]) => ['-', '-', parts.join('-')].join('');
  const promptTool = flag('permission', 'prompt', 'tool');

  test.each([flag('settings'), flag('setting', 'sources'), flag('allowedTools'), flag('allowed', 'tools'), flag('add', 'dir')])(
    '%s is forbidden in the engine',
    (name) => {
      expectViolation(CLAUDE_LAUNCH, argv(name, '/tmp'), 'claude-settings-flags');
      expectViolation(CLAUDE_SESSION, `let arg = format!("${name}={value}");\n`, 'claude-settings-flags');
    },
  );

  test('a prompt tool other than stdio is forbidden in the engine', () => {
    expectViolation(CLAUDE_LAUNCH, argv(promptTool, 'mcp__approve__ask'), 'claude-prompt-tool');
    expectViolation(CLAUDE_LAUNCH, `const ARGS: [&str; 2] = [\n    "${promptTool}",\n    "mcp__x",\n];\n`, 'claude-prompt-tool');
    expectViolation(CLAUDE_SESSION, `let arg = "${promptTool}=mcp__x";\n`, 'claude-prompt-tool');
  });

  test('stdio as the prompt tool passes', () => {
    const allowed = [
      `const ARGS: [&str; 2] = [\n    "${promptTool}",\n    "stdio",\n];\n`,
      `let arg = "${promptTool}=stdio";\n`,
      `//! prompts arrive as control requests (\`${promptTool} stdio\`)\n`,
    ].join('');

    expectClean(CLAUDE_LAUNCH, allowed);
  });

  test('the rules stay out of the rest of the code', () => {
    expectClean('business/src/other/sample.rs', argv(flag('add', 'dir'), '/tmp'));
  });
});

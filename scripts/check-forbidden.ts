// Guard: reject security-sensitive patterns in authored source.
//
// Rules are data (see RULES). A line may opt out of a rule ONLY when the rule has
// `optOut: true` and the same line carries a marker such as
//   // guard-allow: approve-flag
// (several ids may be comma separated). Rules with `optOut: false` can never be waived.
import { extensionOf, failWith, listFiles, matchesAny, readText } from './git-files';

interface Rule {
  id: string;
  reason: string;
  pattern: RegExp;
  /** File extensions (with dot) the rule scans. */
  extensions: readonly string[];
  /** If set, only paths matching one of these globs are scanned. */
  only?: readonly string[];
  /** Paths matching these globs are skipped for this rule. */
  skip?: readonly string[];
  /**
   * `file`: the pattern (with the `g` flag) runs over the whole file, so it can match across lines
   * (an argv array written one token per line); the violation is reported on the line it starts.
   * Default `line`: the pattern runs on each line.
   */
  scope?: 'line' | 'file';
  optOut: boolean;
}

const SOURCE = ['.rs', '.ts', '.svelte', '.js'] as const;
// The OpenCode chat engine (OpenCode 1 and 2): the loopback server clients, the session engines and the
// harness wiring.
const OPENCODE_ENGINE = [
  'business/src/opencode_server/**',
  'business/src/opencode_server_v2/**',
  'business/src/agent/opencode/**',
  'business/src/agent/opencode_v2/**',
  'business/src/harnesses/opencode/**',
] as const;
// The route allowlists, where a Config route must be read only.
const OPENCODE_ROUTES = ['business/src/opencode_server/routes.rs', 'business/src/opencode_server_v2/routes.rs'] as const;
const UI_ONLY = ['ui/**'] as const;
const TEST_FILES = ['**/*.test.ts', 'ui/tests/**'] as const;
// The only Rust files that may name a shell interpreter (each such line needs `guard-allow: installer-shell`):
// the companion installers and the command hook runner, which runs the command text the user wrote or approved.
const INSTALLER_FILES = ['business/src/ecosystem/installer.rs', 'business/src/hooks/command.rs', 'business/tests/**'] as const;
// A shell interpreter as a whole path segment, any case: after a path separator, `.exe` optional
// (`"/usr/bin/bash"`, `#!/bin/sh`, `\System32\cmd.exe`), or as a quoted `.exe` file name
// (`"PowerShell.exe"`, `'pwsh.exe'`). A bare quoted name such as `"bash"` is not matched: it is also
// an RPC tool name and a Markdown fence language (spawning one is caught by `shell-spawn`).
const SHELL_NAME = String.raw`(?:sh|bash|zsh|dash|fish|cmd|pwsh|powershell)`;
const SEGMENT_END = String.raw`(?![\w.-])`;
const SHELL_PROGRAM = new RegExp(
  String.raw`[/\\]${SHELL_NAME}(?:\.exe)?${SEGMENT_END}|["']${SHELL_NAME}\.exe${SEGMENT_END}`,
  'i',
);
// `Command::new` with a quoted (or raw) shell, bare or as the last segment of a path.
const SHELL_COMMAND = new RegExp(
  String.raw`Command::new\(\s*r?#*"(?:[^"]*[/\\])?${SHELL_NAME}(?:\.exe)?"`,
  'i',
);

// Argv arrays: quoted tokens separated by commas and any whitespace, line breaks included.
const QUOTE = '["\'`]';
const ARG = String.raw`${QUOTE}[^"'\`\n]*${QUOTE}`;
const NEXT = String.raw`\s*,\s*`;
// `first` followed by `last` in one array, with any tokens between them.
function argvChain(first: string, last: string): RegExp {
  return new RegExp(String.raw`${QUOTE}(?:${first})${QUOTE}(?:${NEXT}${ARG})*${NEXT}${QUOTE}(?:${last})${QUOTE}`, 'g');
}
// An array literal that starts with one of `tokens` where argv is built: a call argument
// (`.args(["db", ..])`, `spec(ctx, &["db", ..])`), a field (`args: vec!["db"]`), a `let` binding,
// or `.arg("db")`. Constant lists (`const X: &[&str] = &["db", ..]`) are not argv.
function argvStart(tokens: string): RegExp {
  return new RegExp(
    String.raw`(?:(?:[(,:]|\blet\s+\w+\s*=)\s*&?(?:vec!)?\[|\.arg\()\s*${QUOTE}(?:${tokens})${QUOTE}`,
    'g',
  );
}

// A Rust string literal that contains `body`, on a line that is not a comment.
function inLiteral(body: string): string {
  return String.raw`^(?!\s*//).*?"[^"\n]*(?:${body})`;
}

// The OpenCode routes and flags the app must never use (plan docs/plans/opencode-native-chat.md, "Standing rules").
// Route strings are scanned as literals, so a comment that explains the ban stays allowed.
const OPENCODE_RULES: readonly Rule[] = [
  {
    id: 'opencode-share',
    reason: 'never call the OpenCode share routes (a chat would be published); the engine uses a closed route allowlist',
    pattern: new RegExp(inLiteral(String.raw`(?<!\.local|/usr|/local)/(?:un)?share\b`), 'i'),
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-global-admin',
    reason: 'never call /global/dispose, /global/upgrade or /global/config of the OpenCode server',
    pattern: new RegExp(inLiteral(String.raw`/global/(?:dispose|upgrade|config)\b`), 'i'),
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-auth',
    reason: 'never touch OpenCode credentials: no /auth, OAuth or /provider routes (credentials are a non-goal)',
    pattern: new RegExp(inLiteral(String.raw`/auth\b|oauth|/provider(?:/|["?]|$)`), 'i'),
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-surface',
    reason: 'never call the OpenCode /experimental, /tui or /pty routes',
    pattern: new RegExp(inLiteral(String.raw`/(?:experimental|tui|pty)\b`), 'i'),
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-config-write',
    reason: 'never write OpenCode configuration: /config is read only (GET /config and GET /config/providers)',
    pattern: new RegExp(inLiteral(String.raw`/config(?!/providers\b|["?\s]|$)`), 'i'),
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-config-write',
    reason: 'never write OpenCode configuration: a Config route must use GET',
    // A route variant named Config…, alone or in an or-pattern, whose method is not Get.
    pattern:
      /(?:Self|Route(?:V2)?)::Config\w*\s*(?:\{[^}]*\})?(?:\s*\|\s*(?:Self|Route(?:V2)?)::\w+\s*(?:\{[^}]*\})?)*\s*=>\s*Method::(?!Get\b)/g,
    extensions: ['.rs'],
    only: OPENCODE_ROUTES,
    scope: 'file',
    optOut: false,
  },
  {
    id: 'opencode-mdns',
    reason: 'OpenCode must always start with --mdns=false (a user config could otherwise announce and listen beyond loopback)',
    pattern: /["'`]--mdns(?!=false["'`])/,
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-service',
    reason:
      'never start or use the OpenCode background service: no --service or --stdio flag and no "service" command (the chat runs its own loopback server)',
    // Code lines only, like the route rules: a comment that explains the ban stays allowed.
    pattern: /^(?!\s*\/\/).*?(?:["'`]--(?:service|stdio)(?:["'`]|=)|["'`]service["'`])/,
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-cors',
    reason: 'never pass --cors to OpenCode (a browser origin could then call the chat server)',
    pattern: /^(?!\s*\/\/).*?["'`]--cors(?:["'`]|=)/,
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    optOut: false,
  },
  {
    id: 'opencode-port',
    reason: 'never pin the OpenCode server to port 4096 (a fixed port is shared with the user\'s own server); use --port 0',
    pattern: /--port(?:=|\s+)4096\b|["'`]--port["'`]\s*,\s*["'`]4096["'`]/g,
    extensions: ['.rs'],
    only: OPENCODE_ENGINE,
    scope: 'file',
    optOut: false,
  },
];

const RULES: readonly Rule[] = [
  ...OPENCODE_RULES,
  {
    id: 'auto-approve',
    reason: 'never pass --auto-approve to a harness; approvals stay with the user',
    pattern: /--auto-approve/,
    extensions: SOURCE,
    optOut: false,
  },
  {
    id: 'auto-flag',
    reason: 'never pass --auto to a harness (OpenCode auto-approves every permission); approvals stay with the user',
    pattern: /["'`]--auto(?:["'`]|=)/,
    extensions: SOURCE,
    optOut: false,
  },
  {
    id: 'yolo-flag',
    reason: 'never pass --yolo to a harness; approvals stay with the user',
    pattern: /--yolo\b/,
    extensions: SOURCE,
    optOut: false,
  },
  {
    id: 'skip-permissions-flag',
    reason: 'never pass --dangerously-skip-permissions to a harness; approvals stay with the user',
    pattern: /--dangerously-skip-permissions/,
    extensions: SOURCE,
    optOut: false,
  },
  {
    id: 'login-argv',
    reason: 'never run a harness login command ("auth" or "providers" followed by "login"); credentials are a non-goal',
    pattern: /["'`](?:auth|providers)["'`].*["'`]login["'`]/,
    extensions: ['.rs', '.ts'],
    optOut: false,
  },
  {
    id: 'login-argv',
    reason: 'never run a harness login command ("auth" or "providers" followed by "login"); credentials are a non-goal',
    pattern: argvChain('auth|providers', 'login'),
    extensions: ['.rs', '.ts'],
    scope: 'file',
    optOut: false,
  },
  {
    id: 'mcp-argv',
    reason:
      'never run "mcp auth", "mcp logout", "mcp list" or "mcp debug" of a harness (credentials, connections to every server)',
    pattern: argvChain('mcp', 'auth|logout|list|debug'),
    extensions: ['.rs', '.ts'],
    scope: 'file',
    optOut: false,
  },
  {
    id: 'private-store-argv',
    reason: 'never run a harness "db" or "account" command (its database and account hold credentials)',
    pattern: argvStart('db|account'),
    extensions: ['.rs', '.ts'],
    scope: 'file',
    optOut: false,
  },
  {
    id: 'no-ui-flag',
    reason: 'never pass --no-ui; extension UI requests must reach the user',
    pattern: /["'`]--no-ui["'`]/,
    extensions: SOURCE,
    optOut: false,
  },
  {
    id: 'approve-flag',
    reason: '--approve is only allowed for the explicit PI "trust this project once" action',
    pattern: /["'`]--approve["'`]/,
    extensions: SOURCE,
    optOut: true,
  },
  {
    id: 'login-rpc',
    reason: 'login/credential RPCs are a non-goal and must never be sent',
    pattern: /get_login_providers/,
    extensions: SOURCE,
    optOut: true,
  },
  {
    id: 'login-rpc',
    reason: 'a "login" command/type literal is a non-goal (allowed only to document the deny-list)',
    pattern: /["'`]login["'`]/,
    extensions: ['.rs', '.ts'],
    optOut: true,
  },
  {
    id: 'ask-dialog',
    reason: 'set_ask_dialog must never be sent (allowed only in deny-list code)',
    pattern: /set_ask_dialog/,
    extensions: SOURCE,
    optOut: true,
  },
  {
    id: 'shell-spawn',
    reason: 'never spawn a shell; use a fixed binary with explicit argv',
    pattern: SHELL_COMMAND,
    extensions: ['.rs'],
    optOut: false,
  },
  {
    id: 'installer-shell',
    reason:
      'a shell interpreter may only be named by the companion installers (business/src/ecosystem/installer.rs) and the command hook runner (business/src/hooks/command.rs)',
    pattern: SHELL_PROGRAM,
    extensions: ['.rs'],
    skip: INSTALLER_FILES,
    optOut: false,
  },
  {
    id: 'installer-shell',
    reason: 'every line of the companion installers or the command hook runner that names a shell interpreter must be marked',
    pattern: SHELL_PROGRAM,
    extensions: ['.rs'],
    only: INSTALLER_FILES,
    optOut: true,
  },
  {
    id: 'html-injection',
    reason: '{@html} is only allowed in the sanitized Markdown component',
    pattern: /\{@html\b/,
    extensions: ['.svelte'],
    optOut: true,
  },
  {
    id: 'inner-html',
    reason: 'assigning innerHTML/outerHTML or insertAdjacentHTML bypasses sanitization',
    pattern: /\b(?:inner|outer)HTML\b\s*(?:\+?=(?!=)|:)|\binsertAdjacentHTML\b|\bbind:innerHTML\b/,
    extensions: SOURCE,
    only: UI_ONLY,
    optOut: false,
  },
  {
    id: 'eval',
    reason: 'eval() and new Function() are forbidden (strict CSP)',
    pattern: /\beval\s*\(|\bnew\s+Function\s*\(/,
    extensions: SOURCE,
    only: UI_ONLY,
    optOut: false,
  },
  {
    id: 'insecure-updater',
    reason: 'updater must never allow an insecure transport',
    pattern: /dangerous_?[iI]nsecure_?[tT]ransport_?[pP]rotocol/,
    extensions: [...SOURCE, '.json', '.toml'],
    optOut: false,
  },
  {
    id: 'webview-network',
    reason: 'the webview must not open network connections; go through IPC (explicit exceptions need a marker)',
    pattern: /\bfetch\s*\(|\bXMLHttpRequest\b|\bWebSocket\s*\(|\bEventSource\s*\(/,
    extensions: ['.ts', '.svelte', '.js'],
    only: ['ui/src/**'],
    // The vendored logo (copied from ../harness-useful-logo, never edited) contains a fetch for its optional sound
    // sprite. The app never turns that on (the launch sound plays natively), and the CSP allows no such request.
    skip: [...TEST_FILES, 'ui/src/lib/brand/vendor/harness-logo.js'],
    optOut: true,
  },
];

// Never scanned: this guard (it names every pattern), generated code, fixtures and docs.
const GLOBAL_SKIP = [
  'scripts/check-forbidden.ts',
  'ui/src/lib/ipc/bindings/**',
  'ui/src/lib/ipc/mock/fixtures/**',
  'business/tests/fixtures/**',
  'src-tauri/gen/**',
  'docs/**',
  '**/*.md',
];

const MARKER = /guard-allow:\s*([a-z0-9-]+(?:\s*,\s*[a-z0-9-]+)*)/;

function allowedIds(line: string): Set<string> {
  const match = MARKER.exec(line);
  if (!match) {
    return new Set();
  }

  return new Set(match[1].split(',').map((id) => id.trim()));
}

function appliesTo(rule: Rule, path: string): boolean {
  if (!rule.extensions.includes(extensionOf(path))) {
    return false;
  }
  if (rule.only && !matchesAny(path, rule.only)) {
    return false;
  }

  return !(rule.skip && matchesAny(path, rule.skip));
}

// A set: a line rule and a file rule with the same id may both match one line.
const violations = new Set<string>();

for (const path of listFiles()) {
  if (matchesAny(path, GLOBAL_SKIP)) {
    continue;
  }

  const rules = RULES.filter((rule) => appliesTo(rule, path));
  if (rules.length === 0) {
    continue;
  }

  const text = readText(path);
  const lines = text.split(/\r?\n/);

  const report = (rule: Rule, index: number) => {
    if (rule.optOut && allowedIds(lines[index]).has(rule.id)) {
      return;
    }

    const note = rule.optOut ? ` (opt out with "guard-allow: ${rule.id}")` : ' (no opt-out)';
    violations.add(`${path}:${index + 1} [${rule.id}] ${rule.reason}${note}`);
  };

  lines.forEach((line, index) => {
    for (const rule of rules) {
      if (rule.scope !== 'file' && rule.pattern.test(line)) {
        report(rule, index);
      }
    }
  });

  for (const rule of rules.filter((rule) => rule.scope === 'file')) {
    for (const match of text.matchAll(rule.pattern)) {
      report(rule, text.slice(0, match.index).split('\n').length - 1);
    }
  }
}

if (violations.size > 0) {
  failWith(`check-forbidden: ${violations.size} violation(s)`, [...violations]);
}

console.log(`check-forbidden: ok (${new Set(RULES.map((rule) => rule.id)).size} rules)`);

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
  optOut: boolean;
}

const SOURCE = ['.rs', '.ts', '.svelte', '.js'] as const;
const UI_ONLY = ['ui/**'] as const;
const TEST_FILES = ['**/*.test.ts', 'ui/tests/**'] as const;

const RULES: readonly Rule[] = [
  {
    id: 'auto-approve',
    reason: 'never pass --auto-approve to a harness; approvals stay with the user',
    pattern: /--auto-approve/,
    extensions: SOURCE,
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
    pattern: /Command::new\(\s*"(?:sh|bash|zsh|dash|fish|cmd|cmd\.exe|powershell|powershell\.exe|pwsh)"/,
    extensions: ['.rs'],
    optOut: false,
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

const violations: string[] = [];

for (const path of listFiles()) {
  if (matchesAny(path, GLOBAL_SKIP)) {
    continue;
  }

  const rules = RULES.filter((rule) => appliesTo(rule, path));
  if (rules.length === 0) {
    continue;
  }

  readText(path)
    .split(/\r?\n/)
    .forEach((line, index) => {
      const allowed = allowedIds(line);

      for (const rule of rules) {
        if (!rule.pattern.test(line)) {
          continue;
        }
        if (rule.optOut && allowed.has(rule.id)) {
          continue;
        }

        const note = rule.optOut ? ` (opt out with "guard-allow: ${rule.id}")` : ' (no opt-out)';
        violations.push(`${path}:${index + 1} [${rule.id}] ${rule.reason}${note}`);
      }
    });
}

if (violations.length > 0) {
  failWith(`check-forbidden: ${violations.length} violation(s)`, violations);
}

console.log(`check-forbidden: ok (${new Set(RULES.map((rule) => rule.id)).size} rules)`);

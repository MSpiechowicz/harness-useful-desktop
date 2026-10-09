import EyeOff from '@lucide/svelte/icons/eye-off';
import Bot from '@lucide/svelte/icons/bot';
import GitBranch from '@lucide/svelte/icons/git-branch';
import Globe from '@lucide/svelte/icons/globe';
import HardDrive from '@lucide/svelte/icons/hard-drive';
import ScrollText from '@lucide/svelte/icons/scroll-text';
import ShieldOff from '@lucide/svelte/icons/shield-off';
import ShieldX from '@lucide/svelte/icons/shield-x';

import type { IconComponent } from '$lib/components/icon';

export interface PrivacyFact {
  id: string;
  icon: IconComponent;
  title: string;
  text: string;
}

/** What Harness Desktop does with data. Every statement is backed by the code named in the comment. */
export const PRIVACY_FACTS: PrivacyFact[] = [
  {
    // business/src/model/prefs.rs, window-state plugin, diff-mode.svelte.ts.
    id: 'stored',
    icon: HardDrive,
    title: 'Stored on this device',
    text: 'Only app preferences: theme, motion, intro, default harness, update policy, setup progress, window size and position, diff layout, the folder you last opened, recent project paths (if remembered) and binary paths you chose. They live in your OS app-config folder. No chat content, tokens or passwords are saved.',
  },
  {
    // business/src/security/deny.rs.
    id: 'never-read',
    icon: ShieldX,
    title: 'Never read',
    text: 'Credential stores (auth.json, mcp-auth.json, credentials, .env files), private keys (.pem, .key, .p12, SSH keys), databases (.db, .sqlite), log files, agent blob caches and terminal sessions. The check also follows symlinks.',
  },
  {
    // business/src/security/redact.rs, agent/io.rs (stderr), pi_family/rpc/shared.rs (bash output).
    id: 'redacted',
    icon: EyeOff,
    title: 'Shown redacted',
    text: 'Secret-looking configuration values appear hidden: API keys and tokens, MCP server env and headers, auth and OAuth fields, !command values (never run) and credentials inside URLs. Environment references like $NAME show the name only. Agent error output and shell output are redacted before they reach the screen.',
  },
  {
    // tauri.conf.json CSP (connect-src ipc only), src-tauri/src/access.rs link allowlist.
    id: 'network',
    icon: Globe,
    title: 'Network',
    text: 'The app makes no network requests of its own; the interface can only talk to the app itself. Links open in your browser only when you click them, and only for the project and harness sites. Your harness CLIs talk to their providers exactly as they do in your terminal.',
  },
  {
    id: 'telemetry',
    icon: ShieldOff,
    title: 'No telemetry',
    text: 'No analytics, no tracking, no accounts, no sign-in.',
  },
  {
    // business/src/agent, ui/src/features/chat/Composer.svelte, ApprovalBadge.svelte.
    id: 'agents',
    icon: Bot,
    title: 'Agents',
    text: 'Agents run on your machine with your environment and configuration. Harness Desktop never changes your approval mode and shows the active one in the chat header when the harness has one. Shell commands from the composer always ask for confirmation. This app uploads nothing.',
  },
  {
    // business/src/git/cli.rs, business/src/git/mod.rs.
    id: 'git',
    icon: GitBranch,
    title: 'Git',
    text: 'The Changes view runs your local git read-only (status, diff, branch list) with pagers, external diff programs and filesystem monitors off, and with credential prompts disabled.',
  },
  {
    // src-tauri/src/logging.rs.
    id: 'logs',
    icon: ScrollText,
    title: 'Logs',
    text: 'A local log file in your OS log folder records what the app does, never what you say. It holds no prompts, chat content or file contents, and is rotated at 1 MiB.',
  },
];

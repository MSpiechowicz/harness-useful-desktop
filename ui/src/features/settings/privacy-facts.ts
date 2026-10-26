import EyeOff from '@lucide/svelte/icons/eye-off';
import Bot from '@lucide/svelte/icons/bot';
import GitBranch from '@lucide/svelte/icons/git-branch';
import Globe from '@lucide/svelte/icons/globe';
import HardDrive from '@lucide/svelte/icons/hard-drive';
import ScrollText from '@lucide/svelte/icons/scroll-text';
import ShieldOff from '@lucide/svelte/icons/shield-off';
import ShieldX from '@lucide/svelte/icons/shield-x';

import type { IconComponent } from '$lib/components/icon';
import { t } from '$lib/i18n/i18n.svelte';

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
    get title() {
      return t('settings.privacy.stored.title');
    },
    get text() {
      return t('settings.privacy.stored.text');
    },
  },
  {
    // business/src/security/deny.rs.
    id: 'never-read',
    icon: ShieldX,
    get title() {
      return t('settings.privacy.neverRead.title');
    },
    get text() {
      return t('settings.privacy.neverRead.text');
    },
  },
  {
    // business/src/security/redact.rs, agent/io.rs (stderr), pi_family/rpc/shared.rs (bash output).
    id: 'redacted',
    icon: EyeOff,
    get title() {
      return t('settings.privacy.redacted.title');
    },
    get text() {
      return t('settings.privacy.redacted.text');
    },
  },
  {
    // business/src/update.rs, src-tauri/src/commands/updates.rs (https only), tauri.conf.json CSP (connect-src ipc only),
    // src-tauri/src/access.rs link allowlist, ui/src/lib/ipc/actions.ts, the ecosystem_releases command (companion
    // release lookups, gated by checkCompanionUpdates), business/src/git/sync.rs and branch-sync.svelte.ts (the background
    // fetch, gated by backgroundFetch). Same facts as docs/security-privacy.md#network.
    id: 'network',
    icon: Globe,
    get title() {
      return t('settings.privacy.network.title');
    },
    get text() {
      return t('settings.privacy.network.text');
    },
  },
  {
    id: 'telemetry',
    icon: ShieldOff,
    get title() {
      return t('settings.privacy.telemetry.title');
    },
    get text() {
      return t('settings.privacy.telemetry.text');
    },
  },
  {
    // business/src/agent, ui/src/features/chat/Composer.svelte, ApprovalBadge.svelte. The approval override is per session.
    id: 'agents',
    icon: Bot,
    get title() {
      return t('settings.privacy.agents.title');
    },
    get text() {
      return t('settings.privacy.agents.text');
    },
  },
  {
    // business/src/git/cli.rs, business/src/git/mod.rs, business/src/git/sync.rs, business/src/git/branch_ops.rs.
    id: 'git',
    icon: GitBranch,
    get title() {
      return t('settings.privacy.git.title');
    },
    get text() {
      return t('settings.privacy.git.text');
    },
  },
  {
    // src-tauri/src/logging.rs.
    id: 'logs',
    icon: ScrollText,
    get title() {
      return t('settings.privacy.logs.title');
    },
    get text() {
      return t('settings.privacy.logs.text');
    },
  },
];

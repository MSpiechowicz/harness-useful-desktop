import Blocks from '@lucide/svelte/icons/blocks';
import FileDiff from '@lucide/svelte/icons/file-diff';
import GitBranch from '@lucide/svelte/icons/git-branch';
import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
import Slash from '@lucide/svelte/icons/slash';

import { childrenOf, scalarText } from '$lib/components/config-tree';
import type { PaletteCommand } from '$lib/components/palette';
import { statusName } from '$lib/git/entries';
import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { project } from '$lib/stores/project.svelte';
import { resumeInChat } from '$lib/stores/session-actions';
import { sessionList } from '$lib/stores/sessions.svelte';
import { configView, extensionsView } from '$lib/stores/views.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { relativeTime } from '$lib/utils/format';

import { insertSlashCommand } from '../actions';
import { buildItems, TABS, visibleTabs } from '../../extensions/extension-items';
import { sessionTitle } from '../../sessions/session-format';
import { LAYER_TITLES } from '../../config/sections';

/** The saved sessions of the project: Enter opens them in Sessions, Shift+Enter resumes them in chat. */
export function sessionCommands(): PaletteCommand[] {
  const path = project.active?.path;

  return (sessionList.data ?? []).map((summary) => ({
    id: `session-${summary.ref.id}`,
    label: sessionTitle(summary),
    detail: [relativeTime(summary.updatedAt), summary.model].filter(Boolean).join(' · '),
    keywords: summary.firstPrompt ?? '',
    group: 'Sessions',
    kind: 'session',
    icon: GitBranch,
    run: () => {
      viewState.sessionId = summary.ref.id;
      nav.go('sessions');
    },
    secondary: path ? { label: 'Resume in chat', run: () => void resumeInChat(agentSessions, summary.ref, path) } : undefined,
  }));
}

/** The changed files of the project: opens Changes with the file selected. */
export function fileCommands(): PaletteCommand[] {
  return (git.status?.entries ?? []).map((entry) => {
    const status = entry.unstaged === 'untracked' ? 'untracked' : (entry.unstaged ?? entry.staged);

    return {
      id: `file-${entry.path}`,
      label: entry.path,
      detail: status ? (status === 'untracked' ? 'Untracked' : statusName(status)) : '',
      group: 'Changes',
      kind: 'file',
      icon: FileDiff,
      run: () => {
        git.reveal(entry.path);
        nav.go('changes');
      },
    };
  });
}

const MAX_CONFIG_KEYS = 400;
const MAX_KEY_DEPTH = 4;

interface KeyHit {
  path: string;
  leaf: string;
  value: string;
}

function collectKeys(fields: ConfigField[], parents: string[], out: KeyHit[]): void {
  for (const field of fields) {
    if (out.length >= MAX_CONFIG_KEYS) {
      return;
    }

    const path = [...parents, field.key];
    out.push({ path: path.join('.'), leaf: field.key, value: (scalarText(field.value) ?? '').slice(0, 80) });

    if (path.length < MAX_KEY_DEPTH) {
      collectKeys(childrenOf(field.value), path, out);
    }
  }
}

/** Every configuration key (secrets stay hidden): opens Config on that file, filtered to the key. */
export function configCommands(): PaletteCommand[] {
  const view = configView.data;
  if (!view || view.harness !== harness.active?.info.id) {
    return [];
  }

  return view.sections.flatMap((section) => {
    const hits: KeyHit[] = [];
    collectKeys(section.entries, [], hits);

    return hits.map<PaletteCommand>((hit) => ({
      id: `config-${section.id}-${hit.path}`,
      label: hit.path,
      detail: `${section.title} · ${LAYER_TITLES[section.source.layer]}`,
      keywords: hit.value,
      group: 'Config',
      kind: 'config',
      icon: SlidersHorizontal,
      run: () => {
        viewState.configSection = section.id;
        viewState.configQuery = hit.leaf;
        nav.go('config');
      },
    }));
  });
}

/** Plugins, skills, MCP servers and the rest: opens Extensions on that item. */
export function extensionCommands(): PaletteCommand[] {
  const view = extensionsView.data;
  const active = harness.active;
  if (!view || !active || view.harness !== active.info.id) {
    return [];
  }

  return visibleTabs(active.capabilities).flatMap((tab) =>
    buildItems(tab.id, view).map<PaletteCommand>((item) => ({
      id: `extension-${tab.id}-${item.id}`,
      label: item.title,
      detail: TABS.find((candidate) => candidate.id === tab.id)?.label.replace(/s$/, '') ?? tab.label,
      keywords: [item.subtitle, ...item.badges.map((badge) => badge.label)].filter(Boolean).join(' '),
      group: 'Extensions',
      kind: 'extension',
      icon: Blocks,
      run: () => {
        viewState.extensionsTab = tab.id;
        viewState.extensionsItem = item.id;
        viewState.extensionsQuery = '';
        nav.go('extensions');
      },
    })),
  );
}

/** The slash commands of the chat that is open: puts one in its composer. */
export function slashCommands(): PaletteCommand[] {
  const session = agentSessions.active;

  return (session?.commands ?? []).map((spec) => ({
    id: `slash-${spec.name}`,
    label: `/${spec.name}`,
    detail: spec.description,
    keywords: spec.argumentHint ?? '',
    group: 'Slash commands',
    kind: 'slash',
    icon: Slash,
    run: () => insertSlashCommand(spec.name),
  }));
}

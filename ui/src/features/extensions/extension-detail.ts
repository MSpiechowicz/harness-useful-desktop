import { t } from '$lib/i18n/i18n.svelte';
import { formatCount } from '$lib/utils/format';
import { formatDateTime, relativeTime } from '$lib/utils/format';

import { configStats } from '../config/stats';
import type { ExtItem } from './extension-items';
import { agentScope, modeLabel, scopeLabel } from './extension-items';

/** One labelled line of a detail pane. */
export interface Fact {
  label: string;
  value: string;
  mono?: boolean;
  /** An absolute path: shown shortened, in the Location card. */
  path?: boolean;
  /** Where the fact lives: the Location card holds identifiers and paths, the Details card everything else. */
  location?: boolean;
  /** The full text for the tooltip when the value shown is a shortened form of it. */
  title?: string;
  /** A switched-on state, drawn with a green dot. */
  on?: boolean;
}

export interface DetailBasics {
  description: string | null;
  facts: Fact[];
}

function enabledText(enabled: boolean): string {
  return t(enabled ? 'extensions.state.enabled' : 'extensions.state.disabled');
}

function present(facts: (Fact | null)[]): Fact[] {
  return facts.filter((fact): fact is Fact => fact !== null);
}

function optional(label: string, value: string | null, options: Partial<Fact> = {}): Fact | null {
  return value ? { label, value, ...options } : null;
}

/** A timestamp as people read it: `4 weeks ago`, with the exact date and time in the tooltip; anything unparsable stays as written. */
function when(label: string, iso: string | null): Fact | null {
  const date = iso ? formatDateTime(iso) : '';

  return iso ? (date ? { label, value: relativeTime(iso), title: date } : { label, value: iso }) : null;
}

/** The description and key facts of an item, independent of how they are rendered. */
export function detailBasics(entry: ExtItem): DetailBasics {
  switch (entry.tab) {
    case 'plugins': {
      const plugin = entry.data;

      return {
        description: plugin.description,
        facts: present([
          { label: t('extensions.fact.status'), value: enabledText(plugin.enabled), on: plugin.enabled },
          { label: t('extensions.fact.scope'), value: scopeLabel(plugin.scope) },
          optional(t('extensions.fact.version'), plugin.version),
          plugin.updateAvailable ? { label: t('extensions.fact.update'), value: plugin.latestVersion ? t('extensions.badge.updateTo', { version: plugin.latestVersion }) : t('extensions.badge.updateAvailable') } : null,
          optional(t('extensions.fact.marketplace'), plugin.marketplace),
          { label: t('extensions.fact.identifier'), value: plugin.id, mono: true, location: true },
          optional(t('extensions.fact.installDir'), plugin.path, { mono: true, path: true, location: true }),
        ]),
      };
    }

    case 'marketplaces': {
      const marketplace = entry.data;

      return {
        description: null,
        facts: present([
          { label: t('extensions.fact.sourceType'), value: marketplace.sourceType },
          { label: t('extensions.fact.source'), value: marketplace.source, mono: true, location: true },
          when(t('extensions.fact.added'), marketplace.addedAt),
          when(t('extensions.fact.refreshed'), marketplace.updatedAt),
        ]),
      };
    }

    case 'skills': {
      const skill = entry.data;

      return {
        description: skill.description,
        facts: [
          { label: t('extensions.fact.scope'), value: scopeLabel(skill.scope) },
          { label: t('extensions.fact.origin'), value: skill.origin },
          { label: t('extensions.fact.file'), value: skill.path, mono: true, path: true, location: true },
        ],
      };
    }

    case 'prompts': {
      const prompt = entry.data;

      return {
        description: prompt.description,
        facts: present([
          optional(t('extensions.fact.arguments'), prompt.argumentHint, { mono: true }),
          { label: t('extensions.fact.scope'), value: scopeLabel(prompt.scope) },
          { label: t('extensions.fact.file'), value: prompt.path, mono: true, path: true, location: true },
        ]),
      };
    }

    case 'agents': {
      const agent = entry.data;

      return {
        description: agent.description,
        facts: present([
          { label: t('extensions.fact.mode'), value: modeLabel(agent.mode) },
          optional(t('extensions.fact.model'), agent.model, { mono: true }),
          { label: t('extensions.fact.scope'), value: scopeLabel(agentScope(agent)) },
          { label: t('extensions.fact.visibility'), value: t(agent.hidden ? 'extensions.state.hidden' : 'extensions.state.visible') },
          optional(t('extensions.fact.file'), agent.path, { mono: true, path: true, location: true }),
        ]),
      };
    }

    case 'mcp': {
      const server = entry.data;

      return {
        description: null,
        facts: present([
          { label: t('extensions.fact.transport'), value: server.transport },
          { label: t('extensions.fact.status'), value: enabledText(server.enabled), on: server.enabled },
          optional(t('extensions.fact.command'), server.command, { mono: true }),
          { label: t('extensions.fact.definedIn'), value: server.source.path, mono: true, path: server.source.exists, location: true },
        ]),
      };
    }

    case 'themes': {
      const theme = entry.data;

      return {
        description: theme.active ? t('extensions.fact.themeActive') : null,
        facts: present([{ label: t('extensions.fact.scope'), value: scopeLabel(theme.scope) }, optional(t('extensions.fact.file'), theme.path, { mono: true, path: true, location: true })]),
      };
    }

    case 'packages': {
      const pkg = entry.data;

      return {
        description: null,
        facts: [
          { label: t('extensions.fact.kind'), value: pkg.kind },
          { label: t('extensions.fact.scope'), value: scopeLabel(pkg.scope) },
        ],
      };
    }

    case 'modules': {
      const module = entry.data;

      return {
        description: null,
        facts: [
          { label: t('extensions.fact.status'), value: enabledText(module.enabled), on: module.enabled },
          { label: t('extensions.fact.scope'), value: scopeLabel(module.scope) },
          { label: t('extensions.fact.path'), value: module.path, mono: true, path: true, location: true },
        ],
      };
    }
  }
}

/** One number of the Overview card. */
export interface Stat {
  label: string;
  value: string;
}

const NONE = '—';

/** Counts that say what an item holds; kinds with nothing countable (a skill is one file) have none, and show no card. */
export function overviewStats(entry: ExtItem): Stat[] {
  const count = (label: string, value: number): Stat => ({ label, value: formatCount(value) });

  switch (entry.tab) {
    case 'plugins': {
      const plugin = entry.data;

      // A plugin that lists no features (OpenCode's) has no counts worth showing.
      if (plugin.features.length === 0 && !plugin.updateAvailable) {
        return [];
      }

      const on = plugin.features.filter((feature) => feature.enabled).length;
      const update = plugin.updateAvailable ? (plugin.latestVersion ? `v${plugin.latestVersion}` : t('extensions.badge.updateAvailable')) : t('extensions.overview.upToDate');

      return [
        count(t('extensions.plugin.features'), plugin.features.length),
        count(t('extensions.state.enabled'), on),
        count(t('extensions.state.disabled'), plugin.features.length - on),
        { label: t('extensions.fact.update'), value: update },
      ];
    }

    case 'marketplaces': {
      const marketplace = entry.data;
      const installed = marketplace.entries.filter((candidate) => candidate.installed).length;

      return [
        count(t('extensions.tab.plugins'), marketplace.entries.length),
        count(t('extensions.catalog.installed'), installed),
        count(t('extensions.overview.available'), marketplace.entries.length - installed),
        { label: t('extensions.catalog.label'), value: marketplace.error ? t('extensions.badge.unreadable') : t('extensions.overview.readable') },
      ];
    }

    case 'mcp': {
      const server = entry.data;
      const settings = configStats(server.details);
      const hidden = settings.secrets + server.args.filter((arg) => arg.kind === 'secret').length + (server.url?.kind === 'secret' ? 1 : 0);

      return [
        count(t('extensions.mcp.arguments'), server.args.length),
        count(t('extensions.mcp.settings'), settings.entries),
        count(t('extensions.overview.hidden'), hidden),
        { label: t('extensions.overview.depth'), value: settings.entries === 0 ? NONE : formatCount(settings.depth) },
      ];
    }

    case 'skills':
    case 'prompts':
    case 'agents':
    case 'themes':
    case 'packages':
    case 'modules':
      return [];
  }
}

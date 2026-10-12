import { t } from '$lib/i18n/i18n.svelte';
import { formatDateTime, relativeTime } from '$lib/utils/format';

import type { ExtItem } from './extension-items';
import { scopeLabel } from './extension-items';

/** One labelled line of a detail pane. */
export interface Fact {
  label: string;
  value: string;
  mono?: boolean;
  /** Offer "Reveal in file manager" for this value (it is an absolute path). */
  path?: boolean;
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

/** A timestamp as people read it, `Sep 14, 2026, 12:21 PM · 4 weeks ago`; anything unparsable stays as written. */
function when(label: string, iso: string | null): Fact | null {
  const date = iso ? formatDateTime(iso) : '';

  return iso ? { label, value: date ? `${date} · ${relativeTime(iso)}` : iso } : null;
}

/** The description and key facts of an item, independent of how they are rendered. */
export function detailBasics(entry: ExtItem): DetailBasics {
  switch (entry.tab) {
    case 'plugins': {
      const plugin = entry.data;

      return {
        description: plugin.description,
        facts: present([
          { label: t('extensions.fact.identifier'), value: plugin.id, mono: true },
          optional(t('extensions.fact.version'), plugin.version),
          { label: t('extensions.fact.scope'), value: scopeLabel(plugin.scope) },
          optional(t('extensions.fact.marketplace'), plugin.marketplace),
          optional(t('extensions.fact.installDir'), plugin.path, { mono: true, path: true }),
        ]),
      };
    }

    case 'marketplaces': {
      const marketplace = entry.data;

      return {
        description: null,
        facts: present([
          { label: t('extensions.fact.sourceType'), value: marketplace.sourceType },
          { label: t('extensions.fact.source'), value: marketplace.source, mono: true },
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
          { label: t('extensions.fact.file'), value: skill.path, mono: true, path: true },
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
          { label: t('extensions.fact.file'), value: prompt.path, mono: true, path: true },
        ]),
      };
    }

    case 'mcp': {
      const server = entry.data;

      return {
        description: null,
        facts: present([
          { label: t('extensions.fact.transport'), value: server.transport },
          { label: t('extensions.fact.status'), value: enabledText(server.enabled) },
          optional(t('extensions.fact.command'), server.command, { mono: true }),
          { label: t('extensions.fact.definedIn'), value: server.source.path, mono: true, path: server.source.exists },
        ]),
      };
    }

    case 'themes': {
      const theme = entry.data;

      return {
        description: theme.active ? t('extensions.fact.themeActive') : null,
        facts: present([{ label: t('extensions.fact.scope'), value: scopeLabel(theme.scope) }, optional(t('extensions.fact.file'), theme.path, { mono: true, path: true })]),
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
          { label: t('extensions.fact.status'), value: enabledText(module.enabled) },
          { label: t('extensions.fact.scope'), value: scopeLabel(module.scope) },
          { label: t('extensions.fact.path'), value: module.path, mono: true, path: true },
        ],
      };
    }
  }
}

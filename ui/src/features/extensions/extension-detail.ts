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

function present(facts: (Fact | null)[]): Fact[] {
  return facts.filter((fact): fact is Fact => fact !== null);
}

function optional(label: string, value: string | null, options: Partial<Fact> = {}): Fact | null {
  return value ? { label, value, ...options } : null;
}

/** The description and key facts of an item, independent of how they are rendered. */
export function detailBasics(entry: ExtItem): DetailBasics {
  switch (entry.tab) {
    case 'plugins': {
      const plugin = entry.data;

      return {
        description: plugin.description,
        facts: present([
          { label: 'Identifier', value: plugin.id, mono: true },
          optional('Version', plugin.version),
          { label: 'Scope', value: scopeLabel(plugin.scope) },
          optional('Marketplace', plugin.marketplace),
          optional('Install directory', plugin.path, { mono: true, path: true }),
        ]),
      };
    }

    case 'marketplaces': {
      const marketplace = entry.data;

      return {
        description: null,
        facts: present([
          { label: 'Source type', value: marketplace.sourceType },
          { label: 'Source', value: marketplace.source, mono: true },
          optional('Added', marketplace.addedAt),
          optional('Catalog refreshed', marketplace.updatedAt),
        ]),
      };
    }

    case 'skills': {
      const skill = entry.data;

      return {
        description: skill.description,
        facts: [
          { label: 'Scope', value: scopeLabel(skill.scope) },
          { label: 'Origin', value: skill.origin },
          { label: 'File', value: skill.path, mono: true, path: true },
        ],
      };
    }

    case 'prompts': {
      const prompt = entry.data;

      return {
        description: prompt.description,
        facts: present([
          optional('Arguments', prompt.argumentHint, { mono: true }),
          { label: 'Scope', value: scopeLabel(prompt.scope) },
          { label: 'File', value: prompt.path, mono: true, path: true },
        ]),
      };
    }

    case 'mcp': {
      const server = entry.data;

      return {
        description: null,
        facts: present([
          { label: 'Transport', value: server.transport },
          { label: 'Status', value: server.enabled ? 'Enabled' : 'Disabled' },
          optional('Command', server.command, { mono: true }),
          { label: 'Defined in', value: server.source.path, mono: true, path: server.source.exists },
        ]),
      };
    }

    case 'themes': {
      const theme = entry.data;

      return {
        description: theme.active ? 'This theme is currently selected.' : null,
        facts: present([{ label: 'Scope', value: scopeLabel(theme.scope) }, optional('File', theme.path, { mono: true, path: true })]),
      };
    }

    case 'packages': {
      const pkg = entry.data;

      return {
        description: null,
        facts: [
          { label: 'Kind', value: pkg.kind },
          { label: 'Scope', value: scopeLabel(pkg.scope) },
        ],
      };
    }

    case 'modules': {
      const module = entry.data;

      return {
        description: null,
        facts: [
          { label: 'Status', value: module.enabled ? 'Enabled' : 'Disabled' },
          { label: 'Scope', value: scopeLabel(module.scope) },
          { label: 'Path', value: module.path, mono: true, path: true },
        ],
      };
    }
  }
}

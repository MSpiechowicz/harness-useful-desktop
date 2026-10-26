import { t } from '$lib/i18n/i18n.svelte';
import type { ConfigFormat } from '$lib/ipc/bindings/ConfigFormat';
import type { ConfigLayer } from '$lib/ipc/bindings/ConfigLayer';
import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
import { countMatches } from '$lib/components/config-tree';

/** Getters, so a title follows the language on screen. */
export const LAYER_TITLES: Record<ConfigLayer, string> = {
  get user() {
    return t('config.layer.user');
  },
  get profile() {
    return t('config.layer.profile');
  },
  get project() {
    return t('config.layer.project');
  },
};

export const FORMAT_LABELS: Record<ConfigFormat, string> = {
  yaml: 'YAML',
  json: 'JSON',
  jsonc: 'JSONC',
  toml: 'TOML',
  markdown: 'Markdown',
  get text() {
    return t('config.format.text');
  },
};

/** The short mark on a file's row in the list; the same in every language. */
export const FORMAT_TILES: Record<ConfigFormat, string> = {
  yaml: 'YAML',
  json: 'JSON',
  jsonc: 'JSONC',
  toml: 'TOML',
  markdown: 'MD',
  text: 'TXT',
};

const LAYER_ORDER: ConfigLayer[] = ['user', 'profile', 'project'];

export interface SectionGroup {
  layer: ConfigLayer;
  title: string;
  sections: ConfigSection[];
}

/** Sections grouped by layer in User, Profile, Project order; source order is kept inside a group. */
export function groupSections(sections: ConfigSection[]): SectionGroup[] {
  return LAYER_ORDER.map((layer) => ({
    layer,
    title: LAYER_TITLES[layer],
    sections: sections.filter((section) => section.source.layer === layer),
  })).filter((group) => group.sections.length > 0);
}

/** The file name of a path, for either path separator. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/** The sections a search leaves in the list: those whose title, file name or description match, or whose content has a matching key or value. */
export function filterSections(sections: ConfigSection[], query: string): ConfigSection[] {
  const needle = query.trim().toLowerCase();
  if (needle === '') {
    return sections;
  }

  return sections.filter(
    (section) =>
      [section.title, section.description ?? '', baseName(section.source.path)].some((text) => text.toLowerCase().includes(needle)) || countMatches(section.entries, needle) > 0,
  );
}

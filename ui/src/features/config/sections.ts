import { t } from '$lib/i18n/i18n.svelte';
import type { ConfigFormat } from '$lib/ipc/bindings/ConfigFormat';
import type { ConfigLayer } from '$lib/ipc/bindings/ConfigLayer';
import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';

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
  markdown: 'Markdown',
  get text() {
    return t('config.format.text');
  },
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

import type { ConfigFormat } from '$lib/ipc/bindings/ConfigFormat';
import type { ConfigLayer } from '$lib/ipc/bindings/ConfigLayer';
import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';

export const LAYER_TITLES: Record<ConfigLayer, string> = {
  user: 'User',
  profile: 'Profile',
  project: 'Project',
};

export const FORMAT_LABELS: Record<ConfigFormat, string> = {
  yaml: 'YAML',
  json: 'JSON',
  markdown: 'Markdown',
  text: 'Text',
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

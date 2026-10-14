import type { Capabilities } from '$lib/ipc/bindings/Capabilities';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';

import { buildItems, filterItems, visibleTabs, type ExtItem, type ExtTab } from './extension-items';

/** One heading of the inventory with the items that match the search. */
export interface ExtGroup {
  tab: ExtTab;
  label: string;
  items: ExtItem[];
  /** How many items the group holds before the search; a folded group still says so. */
  total: number;
}

/** The kinds this harness has, in display order, each with its matching items; kinds with nothing to show are absent. */
export function buildGroups(view: ExtensionsView, capabilities: Capabilities, query: string): ExtGroup[] {
  return visibleTabs(capabilities).flatMap((def) => {
    const all = buildItems(def.id, view);
    const items = filterItems(all, query);

    return items.length === 0 ? [] : [{ tab: def.id, label: def.label, items, total: all.length }];
  });
}

/** The chosen item, or the first one shown while nothing (or something that is gone) is chosen. */
export function pickItem(groups: ExtGroup[], tab: string | null, id: string | null): ExtItem | undefined {
  const items = groups.flatMap((group) => group.items);

  return items.find((entry) => entry.tab === tab && entry.id === id) ?? items[0];
}

/** The file or folder "Reveal" opens for an item; none for items that live nowhere on disk (a marketplace by URL). */
export function revealTarget(entry: ExtItem): string | null {
  switch (entry.tab) {
    case 'plugins':
    case 'skills':
    case 'prompts':
    case 'modules':
    case 'themes':
      return entry.data.path;
    case 'mcp':
      return entry.data.source.exists ? entry.data.source.path : null;
    case 'marketplaces':
    case 'packages':
      return null;
  }
}

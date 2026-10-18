import { t } from '$lib/i18n/i18n.svelte';

type EntryKey = 'toolbar.installPlugin' | 'toolbar.addMarketplace' | 'toolbar.installPackage' | 'title.upgradeAll' | 'title.updateAllPackages';

/** A menu entry whose label follows the language on screen. */
export function entry<T extends object>(key: EntryKey, rest: T): T & { readonly label: string } {
  return {
    get label() {
      return t(`extensions.${key}`);
    },
    ...rest,
  };
}

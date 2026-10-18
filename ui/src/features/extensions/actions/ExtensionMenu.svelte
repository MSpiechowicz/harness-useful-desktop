<script lang="ts">
  import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
  import FolderSearch from '@lucide/svelte/icons/folder-search';

  import type { IconComponent } from '$lib/components/icon';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { revealPath } from '$lib/ipc/actions';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';

  import { withCompanionUpdates } from '../companion-updates';
  import { actionRequest, type UserAction } from './action-model';
  import { actionRunner } from './action-runner.svelte';
  import { entry } from './entry';

  interface Entry {
    readonly label: string;
    icon: IconComponent;
    /** Needs this capability of the harness. */
    needs: 'plugins' | 'packages';
    action: UserAction;
    /** Greys the entry out while there is nothing for it to do. */
    idle?: () => boolean;
  }

  // The "⋯" menu keeps the bulk actions, whatever item is selected.
  const BULK: Entry[] = [
    entry('title.upgradeAll', {
      icon: ArrowUpFromLine,
      needs: 'plugins',
      action: { type: 'pluginUpgrade', name: null },
      idle: () => !(extensionsView.data && withCompanionUpdates(extensionsView.data, ecosystem.status).plugins.some((plugin) => plugin.updateAvailable)),
    }),
    entry('title.updateAllPackages', { icon: ArrowUpFromLine, needs: 'packages', action: { type: 'packageUpdate', source: null } }),
  ];

  interface Props {
    /** The file or folder of the selected item, offered as "Reveal in file manager"; none for an item with no place on disk. */
    reveal?: string | null;
  }

  let { reveal = null }: Props = $props();

  const capabilities = $derived(harness.active?.capabilities);
  const bulk = $derived(BULK.filter((item) => capabilities?.cliActions && capabilities[item.needs]));
  const busy = $derived(actionRunner.phase !== 'idle');

  function run(item: Entry, close: () => void): void {
    const active = harness.active;

    close();

    if (active) {
      void actionRunner.begin(actionRequest(active.info.id, item.action, 'user', null));
    }
  }
</script>

{#if bulk.length > 0 || reveal}
  <MoreMenu label={t('extensions.menu.label')} panelClass="w-60">
    {#snippet children({ close })}
      {#each bulk as item (item.label)}
        <MenuItem icon={item.icon} disabled={busy || item.idle?.()} onclick={() => run(item, close)}>{item.label}</MenuItem>
      {/each}

      {#if reveal}
        <MenuItem
          icon={FolderSearch}
          onclick={() => {
            close();
            void revealPath(reveal);
          }}
        >
          {t('extensions.reveal')}
        </MenuItem>
      {/if}
    {/snippet}
  </MoreMenu>
{/if}

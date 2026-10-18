<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';

  import DropdownButton from '$lib/components/DropdownButton.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';

  import type { ActionRequest, ComposeKind } from './action-model';
  import { actionRunner } from './action-runner.svelte';
  import { entry } from './entry';
  import InstallDialog from './InstallDialog.svelte';

  interface Entry {
    readonly label: string;
    icon: IconComponent;
    /** Needs this capability of the harness. */
    needs: 'plugins' | 'marketplaces' | 'packages';
    /** Opens the install dialog. */
    compose: ComposeKind;
  }

  // The Install button is the same in every view of the inventory; the menu offers what this harness can add.
  // The bulk actions and "Reveal" live in ExtensionMenu, at the fixed end of the top bar.
  const ADD: Entry[] = [
    entry('toolbar.installPlugin', { icon: Plus, needs: 'plugins', compose: 'plugin' }),
    entry('toolbar.addMarketplace', { icon: Plus, needs: 'marketplaces', compose: 'marketplace' }),
    entry('toolbar.installPackage', { icon: Plus, needs: 'packages', compose: 'package' }),
  ];

  let composing = $state<ComposeKind | null>(null);

  const capabilities = $derived(harness.active?.capabilities);
  // Install, update and remove run through the harness CLI, so a harness the app does not drive that way offers none of them.
  const adds = $derived(ADD.filter((item) => capabilities?.cliActions && capabilities[item.needs]));
  const busy = $derived(actionRunner.phase !== 'idle');

  function compose(item: Entry, close: () => void): void {
    close();
    composing = item.compose;
  }

  function review(request: ActionRequest): void {
    composing = null;
    void actionRunner.begin(request);
  }
</script>

{#if adds.length > 0}
  <Popover label={t('extensions.install.button')} kind="menu" align="end" class="w-52 min-w-0!">
    {#snippet trigger({ props })}
      <DropdownButton {...props} variant="primary" icon={Plus} disabled={busy}>{t('extensions.install.button')}</DropdownButton>
    {/snippet}

    {#snippet children({ close })}
      <div role="none" class="flex flex-col">
        {#each adds as item (item.label)}
          <MenuItem icon={item.icon} disabled={busy} onclick={() => compose(item, close)}>{item.label}</MenuItem>
        {/each}
      </div>
    {/snippet}
  </Popover>
{/if}

{#if composing}
  <InstallDialog kind={composing} onsubmit={review} onclose={() => (composing = null)} />
{/if}

<script lang="ts">
  import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
  import Plus from '@lucide/svelte/icons/plus';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import DropdownButton from '$lib/components/DropdownButton.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';

  import { withCompanionUpdates } from '../companion-updates';
  import { actionRequest, type ActionRequest, type ComposeKind } from './action-model';
  import { actionRunner } from './action-runner.svelte';
  import InstallDialog from './InstallDialog.svelte';

  interface Entry {
    readonly label: string;
    icon: IconComponent;
    /** Needs this capability of the harness. */
    needs: 'plugins' | 'marketplaces' | 'packages';
    /** Opens the install dialog instead of going straight to a confirmation. */
    compose?: ComposeKind;
    action?: ExtensionAction;
    /** Greys the entry out while there is nothing for it to do. */
    idle?: () => boolean;
  }

  type EntryKey = 'toolbar.installPlugin' | 'toolbar.addMarketplace' | 'toolbar.installPackage' | 'title.upgradeAll' | 'title.updateAllMarketplaces' | 'title.updateAllPackages';

  /** An entry whose label follows the language on screen. */
  function entry(key: EntryKey, rest: Omit<Entry, 'label'>): Entry {
    return {
      get label() {
        return t(`extensions.${key}`);
      },
      ...rest,
    };
  }

  // The Install button is the same in every view of the inventory; the menu offers what this harness can add.
  const ADD: Entry[] = [
    entry('toolbar.installPlugin', { icon: Plus, needs: 'plugins', compose: 'plugin' }),
    entry('toolbar.addMarketplace', { icon: Plus, needs: 'marketplaces', compose: 'marketplace' }),
    entry('toolbar.installPackage', { icon: Plus, needs: 'packages', compose: 'package' }),
  ];

  // The "⋯" menu keeps the bulk actions, whatever item is selected.
  const BULK: Entry[] = [
    entry('title.upgradeAll', {
      icon: ArrowUpFromLine,
      needs: 'plugins',
      action: { type: 'pluginUpgrade', name: null },
      idle: () => !(extensionsView.data && withCompanionUpdates(extensionsView.data, ecosystem.status).plugins.some((plugin) => plugin.updateAvailable)),
    }),
    entry('title.updateAllMarketplaces', { icon: RefreshCw, needs: 'marketplaces', action: { type: 'marketplaceUpdate', name: null } }),
    entry('title.updateAllPackages', { icon: ArrowUpFromLine, needs: 'packages', action: { type: 'packageUpdate', source: null } }),
  ];

  let composing = $state<ComposeKind | null>(null);

  const capabilities = $derived(harness.active?.capabilities);
  const adds = $derived(ADD.filter((item) => capabilities?.[item.needs]));
  const bulk = $derived(BULK.filter((item) => capabilities?.[item.needs]));
  const busy = $derived(actionRunner.phase !== 'idle');

  function run(item: Entry, close: () => void): void {
    const active = harness.active;

    close();

    if (item.compose) {
      composing = item.compose;
    } else if (item.action && active) {
      void actionRunner.begin(actionRequest(active.info.id, item.action, 'user', null));
    }
  }

  function review(request: ActionRequest): void {
    composing = null;
    void actionRunner.begin(request);
  }
</script>

{#if adds.length === 1}
  <Button variant="primary" disabled={busy} onclick={() => (composing = adds[0].compose ?? null)}>
    {#snippet leading()}<Plus size={ICON.sm} aria-hidden="true" />{/snippet}
    {adds[0].label}
  </Button>
{:else if adds.length > 1}
  <Popover label={t('extensions.install.button')} kind="menu" align="end" class="w-52 min-w-0!">
    {#snippet trigger({ props })}
      <DropdownButton {...props} variant="primary" icon={Plus} disabled={busy}>{t('extensions.install.button')}</DropdownButton>
    {/snippet}

    {#snippet children({ close })}
      <div role="none" class="flex flex-col">
        {@render entries(adds, close)}
      </div>
    {/snippet}
  </Popover>
{/if}

{#if bulk.length > 0}
  <MoreMenu label={t('extensions.menu.label')} panelClass="w-60">
    {#snippet children({ close })}
      {@render entries(bulk, close)}
    {/snippet}
  </MoreMenu>
{/if}

{#snippet entries(list: Entry[], close: () => void)}
  {#each list as item (item.label)}
    <MenuItem icon={item.icon} disabled={busy || item.idle?.()} onclick={() => run(item, close)}>{item.label}</MenuItem>
  {/each}
{/snippet}

{#if composing}
  <InstallDialog kind={composing} onsubmit={review} onclose={() => (composing = null)} />
{/if}

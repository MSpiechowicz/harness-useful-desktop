<script lang="ts">
  import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
  import Plus from '@lucide/svelte/icons/plus';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';

  import { actionRequest, type ActionRequest, type ComposeKind } from './action-model';
  import { actionRunner } from './action-runner.svelte';
  import InstallDialog from './InstallDialog.svelte';

  interface Props {
    /** The extension tab in view; only some tabs have bulk actions. */
    tab: string | undefined;
  }

  let { tab }: Props = $props();

  interface ToolbarAction {
    readonly label: string;
    icon: IconComponent;
    /** Opens the install dialog instead of going straight to a confirmation. */
    compose?: ComposeKind;
    action?: ExtensionAction;
    primary?: boolean;
    /** Greys the action out while there is nothing for it to do. */
    idle?: () => boolean;
  }

  type ToolbarKey = 'installPlugin' | 'upgradeAll' | 'addMarketplace' | 'updateAll' | 'installPackage';

  /** A toolbar action whose label follows the language on screen. */
  function toolbarAction(key: ToolbarKey, rest: Omit<ToolbarAction, 'label'>): ToolbarAction {
    return {
      get label() {
        return t(`extensions.toolbar.${key}`);
      },
      ...rest,
    };
  }

  const TOOLBARS: Record<string, ToolbarAction[]> = {
    plugins: [
      toolbarAction('installPlugin', { icon: Plus, compose: 'plugin', primary: true }),
      toolbarAction('upgradeAll', {
        icon: ArrowUpFromLine,
        action: { type: 'pluginUpgrade', name: null },
        idle: () => !(extensionsView.data?.plugins.some((plugin) => plugin.updateAvailable) ?? false),
      }),
    ],
    marketplaces: [
      toolbarAction('addMarketplace', { icon: Plus, compose: 'marketplace', primary: true }),
      toolbarAction('updateAll', { icon: RefreshCw, action: { type: 'marketplaceUpdate', name: null } }),
    ],
    packages: [
      toolbarAction('installPackage', { icon: Plus, compose: 'package', primary: true }),
      toolbarAction('updateAll', { icon: RefreshCw, action: { type: 'packageUpdate', source: null } }),
    ],
  };

  let composing = $state<ComposeKind | null>(null);

  const actions = $derived(tab ? (TOOLBARS[tab] ?? []) : []);
  const busy = $derived(actionRunner.phase !== 'idle');

  function run(entry: ToolbarAction): void {
    const active = harness.active;

    if (entry.compose) {
      composing = entry.compose;
    } else if (entry.action && active) {
      void actionRunner.begin(actionRequest(active.info.id, entry.action, 'user', null));
    }
  }

  function review(request: ActionRequest): void {
    composing = null;
    void actionRunner.begin(request);
  }
</script>

{#each actions as entry (entry.label)}
  <Button variant={entry.primary ? 'primary' : 'secondary'} disabled={busy || !harness.active || entry.idle?.()} onclick={() => run(entry)}>
    {#snippet leading()}<entry.icon size={ICON.sm} aria-hidden="true" />{/snippet}
    {entry.label}
  </Button>
{/each}

{#if composing}
  <InstallDialog kind={composing} onsubmit={review} onclose={() => (composing = null)} />
{/if}

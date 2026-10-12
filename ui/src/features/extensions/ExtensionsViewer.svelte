<script lang="ts">
  import Blocks from '@lucide/svelte/icons/blocks';

  import EmptyState from '$lib/components/EmptyState.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import RefreshButton from '$lib/components/RefreshButton.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';

  import HarnessGate from '../harness/HarnessGate.svelte';
  import ActionDialog from './actions/ActionDialog.svelte';
  import ExtensionToolbar from './actions/ExtensionToolbar.svelte';
  import { tabCount, visibleTabs, type ExtTab } from './extension-items';
  import ExtensionsPanel from './ExtensionsPanel.svelte';

  const tabs = $derived(harness.active ? visibleTabs(harness.active.capabilities) : []);
  const currentTab = $derived(tabs.some((tab) => tab.id === viewState.extensionsTab) ? viewState.extensionsTab : tabs[0]?.id);

  function chooseTab(next: ExtTab): void {
    viewState.extensionsTab = next;
    viewState.extensionsItem = null;
    viewState.extensionsQuery = '';
  }
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title={t('extensions.title')} icon={Blocks} context={project.active ? t('extensions.context.project') : t('extensions.context.user')}>
    {#snippet actions()}
      <ExtensionToolbar tab={currentTab ?? undefined} />
      <RefreshButton label={t('extensions.reload')} onclick={() => extensionsView.refresh()} />
    {/snippet}
  </ViewHeader>

  <ActionDialog />

  <div class="relative min-h-0 flex-1">
    <HarnessGate loader={extensionsView} area="extensions">
      {#snippet children(view)}
        {@const options = tabs.map((tab) => ({ value: tab.id, label: tab.label, count: tabCount(tab.id, view) }))}
        {@const current = options.some((option) => option.value === viewState.extensionsTab) ? (viewState.extensionsTab as ExtTab) : options[0]?.value}

        {#if view.notices.length > 0}
          <div class="page-gutter pt-3"><NoticeBanner notices={view.notices} /></div>
        {/if}

        {#if current}
          <Tabs
            items={options}
            bind:value={() => current, chooseTab}
            label={t('extensions.types')}
            class="mt-1 min-h-0 flex-1 gap-0"
            stripClass="mx-[var(--gutter)] max-w-[calc(100%-2*var(--gutter))] self-start overflow-x-auto"
            panelClass="min-h-0 flex-1 border-t border-line"
          >
            {#snippet panel(tab)}
              <ExtensionsPanel {tab} {view} />
            {/snippet}
          </Tabs>
        {:else}
          <div class="grid flex-1 place-items-center">
            <EmptyState icon={Blocks} title={t('extensions.noTypes.title')} body={t('extensions.noTypes.body')} />
          </div>
        {/if}
      {/snippet}
    </HarnessGate>
  </div>
</div>

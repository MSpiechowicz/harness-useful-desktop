<script lang="ts">
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import Blocks from '@lucide/svelte/icons/blocks';
  import PackageOpen from '@lucide/svelte/icons/package-open';

  import DrawerToggle from '$lib/components/DrawerToggle.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ListDetail from '$lib/components/ListDetail.svelte';
  import { PaneDrawer } from '$lib/components/pane-drawer.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';

  import HarnessGate from '../harness/HarnessGate.svelte';
  import { scopeLabel } from '../shell/scope';
  import ActionDialog from './actions/ActionDialog.svelte';
  import ExtensionToolbar from './actions/ExtensionToolbar.svelte';
  import { withCompanionUpdates } from './companion-updates';
  import ExtensionDetail from './ExtensionDetail.svelte';
  import ExtensionList from './ExtensionList.svelte';
  import { buildGroups, pickItem, revealTarget } from './extension-groups';
  import type { ExtItem } from './extension-items';

  $effect(() =>
    autoRefresh(() => {
      void extensionsView.refresh({ quiet: true });
      void ecosystem.load();
    }, 5000),
  );

  // The catalogs OMP keeps are only as new as their last refresh; the companions' GitHub releases add the updates they miss.
  const view = $derived(extensionsView.data && extensionsView.data.harness === harness.active?.info.id ? withCompanionUpdates(extensionsView.data, ecosystem.status) : undefined);
  const query = $derived(viewState.extensionsQuery.trim());
  // The groups before the search say whether there is anything at all; the ones after say what matches.
  const everything = $derived(view && harness.active ? buildGroups(view, harness.active.capabilities, '') : []);
  const groups = $derived(view && harness.active ? buildGroups(view, harness.active.capabilities, viewState.extensionsQuery) : []);
  const selected = $derived(pickItem(groups, viewState.extensionsTab, viewState.extensionsItem));
  const total = $derived(everything.reduce((sum, group) => sum + group.total, 0));
  const target = $derived(selected ? revealTarget(selected) : null);
  const drawer = new PaneDrawer();

  function choose(entry: ExtItem): void {
    viewState.extensionsTab = entry.tab;
    viewState.extensionsItem = entry.id;
    drawer.close();
  }

  function onSearchKey(event: KeyboardEvent): void {
    if (event.key === 'Escape' && viewState.extensionsQuery !== '') {
      event.preventDefault();
      event.stopPropagation();
      viewState.extensionsQuery = '';
    }
  }
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title={t('extensions.title')} icon={Blocks} context={scopeLabel(view ? tn('extensions.count.item', total) : null)}>
    {#snippet actions()}
      {#if total > 0}
        <div class="w-[340px] max-[900px]:w-[220px]">
          <SearchField
            bind:value={viewState.extensionsQuery}
            label={t('extensions.search.label')}
            placeholder={t('extensions.search.placeholder')}
            shortcut="mod+f"
            onkeydown={onSearchKey}
          />
        </div>

        <DrawerToggle {drawer} label={t('extensions.drawer.label')} count={total} />
      {/if}

      <ExtensionToolbar reveal={target} />
    {/snippet}
  </ViewHeader>

  <ActionDialog />

  <div class="relative min-h-0 flex-1">
    <HarnessGate loader={extensionsView} area="extensions">
      {#snippet children(data)}
        {#if data.notices.length > 0}
          <div class="page-gutter border-b border-line py-3"><NoticeBanner notices={data.notices} /></div>
        {/if}

        <div class="mx-auto min-h-0 w-full max-w-[var(--page-wide)] flex-1">
        {#if total === 0}
          <div class="grid size-full place-items-center">
            <EmptyState icon={PackageOpen} title={t('extensions.empty.title')} body={t('extensions.empty.body')} />
          </div>
        {:else}
          <ListDetail label={t('extensions.resize')} scrollKey="extensions:detail" size={288} {drawer} drawerLabel={t('extensions.drawer.label')}>
            {#snippet list()}
              {#if groups.length === 0}
                <p class="px-4 py-6 text-center text-sm text-fg-3" role="status">{t('extensions.search.noMatch', { query })}</p>
              {:else}
                <ExtensionList {groups} {selected} searching={query !== ''} onselect={choose} />
              {/if}
            {/snippet}

            {#snippet detail()}
              {#if selected}
                {#key `${selected.tab}:${selected.id}`}
                  <ExtensionDetail entry={selected} />
                {/key}
              {/if}
            {/snippet}
          </ListDetail>
        {/if}
        </div>
      {/snippet}
    </HarnessGate>
  </div>
</div>

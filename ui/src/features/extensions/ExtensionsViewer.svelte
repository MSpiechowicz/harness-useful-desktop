<script lang="ts">
  import Blocks from '@lucide/svelte/icons/blocks';
  import PackageOpen from '@lucide/svelte/icons/package-open';

  import EmptyState from '$lib/components/EmptyState.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import PaneHeader from '$lib/components/PaneHeader.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import WorkspaceLayout from '$lib/components/WorkspaceLayout.svelte';
  import WorkspaceToggles from '$lib/components/WorkspaceToggles.svelte';
  import { WorkspaceState } from '$lib/components/workspace.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { EXTENSIONS_POLL_MS } from '$lib/stores/live';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';

  import HarnessGate from '../harness/HarnessGate.svelte';
  import HarnessSwitcher from '../harness/HarnessSwitcher.svelte';
  import { scopeLabel } from '../shell/scope';
  import ActionDialog from './actions/ActionDialog.svelte';
  import ExtensionMenu from './actions/ExtensionMenu.svelte';
  import ExtensionToolbar from './actions/ExtensionToolbar.svelte';
  import { withCompanionUpdates } from './companion-updates';
  import ExtensionAside from './ExtensionAside.svelte';
  import ExtensionDetail from './ExtensionDetail.svelte';
  import ExtensionList from './ExtensionList.svelte';
  import { detailSubtitle } from './extension-detail';
  import { buildGroups, pickItem, revealTarget } from './extension-groups';
  import type { ExtItem } from './extension-items';

  // A read that failed is retried on the retry schedule, a little later each time, instead of at the poll's pace.
  $effect(() =>
    autoRefresh(() => Promise.all([extensionsView.refresh({ quiet: true }), ecosystem.load(false, { quiet: true })]), EXTENSIONS_POLL_MS, {
      failing: extensionsView.status === 'error',
    }),
  );

  // The catalogs OMP keeps are only as new as their last refresh; the companions' GitHub releases add the updates they miss.
  // While a switch loads, the header keeps the previous harness's view so it does not jump; the body shows a spinner (HarnessGate).
  const view = $derived(extensionsView.data ? withCompanionUpdates(extensionsView.data, ecosystem.status) : undefined);
  // Mid-switch the header is frozen: it reads only the old view, with that harness's own kinds, and takes no input.
  const switching = $derived(view !== undefined && view.harness !== harness.active?.info.id);
  const capabilities = $derived(view ? harness.get(view.harness)?.capabilities : undefined);
  const query = $derived(viewState.extensionsQuery.trim());
  // The groups before the search say whether there is anything at all; the ones after say what matches.
  const everything = $derived(view && capabilities ? buildGroups(view, capabilities, '') : []);
  const groups = $derived(view && capabilities ? buildGroups(view, capabilities, viewState.extensionsQuery) : []);
  const selected = $derived(pickItem(groups, viewState.extensionsTab, viewState.extensionsItem));
  const total = $derived(everything.reduce((sum, group) => sum + group.total, 0));
  const target = $derived(selected && !switching ? revealTarget(selected) : null);
  const workspace = new WorkspaceState('extensions', { details: true });

  function choose(entry: ExtItem): void {
    viewState.extensionsTab = entry.tab;
    viewState.extensionsItem = entry.id;
    workspace.drawer.close();
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
        <div class="search-width" inert={switching}>
          <SearchField
            bind:value={viewState.extensionsQuery}
            label={t('extensions.search.label')}
            placeholder={t('extensions.search.placeholder')}
            shortcut="mod+f"
            onkeydown={onSearchKey}
          />
        </div>
      {/if}

      <!-- Shown whatever the harness has: a harness with no extensions still has to be switchable away from. -->
      <HarnessSwitcher />

      <div class="contents" inert={switching}>
        <ExtensionToolbar />
      </div>
    {/snippet}

    {#snippet fixed()}
      <div class="contents" inert={switching}>
        <WorkspaceToggles {workspace} listLabel={t('extensions.drawer.label')} />
        <ExtensionMenu reveal={target} />
      </div>
    {/snippet}
  </ViewHeader>

  <ActionDialog />

  <div class="relative min-h-0 flex-1">
    <HarnessGate loader={extensionsView} area="extensions">
      {#snippet children(data)}
        {#if data.notices.length > 0}
          <div class="notice-band border-b border-line py-3"><NoticeBanner notices={data.notices} /></div>
        {/if}

        <div class="min-h-0 flex-1">
          {#if total === 0}
            <div class="grid size-full place-items-center">
              <EmptyState icon={PackageOpen} title={t('extensions.empty.title')} body={t('extensions.empty.body')} />
            </div>
          {:else}
            {#snippet kindLine()}{selected ? detailSubtitle(selected) : ''}{/snippet}

            <WorkspaceLayout
              {workspace}
              listLabel={t('extensions.drawer.label')}
              resizeLabel={t('extensions.resize')}
              scroll="page"
              scrollKey={selected ? `extensions:detail:${selected.tab}:${selected.id}` : 'extensions:detail'}
              detailsSubtitle={selected?.title}
              detailsEmpty={selected ? undefined : { icon: Blocks, message: t('extensions.detail.empty') }}
            >
              {#snippet list()}
                {#if groups.length === 0}
                  <p class="px-4 py-6 text-center text-sm text-fg-3" role="status">{t('extensions.search.noMatch', { query })}</p>
                {:else}
                  <ExtensionList {groups} {selected} searching={query !== ''} onselect={choose} />
                {/if}
              {/snippet}

              {#snippet header()}
                {#if selected}<PaneHeader title={selected.title} titleId="extension-title" subtitle={kindLine} />{/if}
              {/snippet}

              {#snippet main()}
                {#if selected}
                  {#key `${selected.tab}:${selected.id}`}
                    <ExtensionDetail entry={selected} />
                  {/key}
                {/if}
              {/snippet}

              {#snippet details()}
                {#if selected}
                  <ExtensionAside entry={selected} />
                {/if}
              {/snippet}
            </WorkspaceLayout>
          {/if}
        </div>
      {/snippet}
    </HarnessGate>
  </div>
</div>

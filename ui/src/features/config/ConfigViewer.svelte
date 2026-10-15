<script lang="ts">
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import Code from '@lucide/svelte/icons/code';
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';

  import { countMatches } from '$lib/components/config-tree';
  import DrawerToggle from '$lib/components/DrawerToggle.svelte';
  import ListDetail from '$lib/components/ListDetail.svelte';
  import { PaneDrawer } from '$lib/components/pane-drawer.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { ConfigView } from '$lib/ipc/bindings/ConfigView';
  import { revealPath } from '$lib/ipc/actions';
  import { harness } from '$lib/stores/harness.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { configView } from '$lib/stores/views.svelte';

  import HarnessGate from '../harness/HarnessGate.svelte';
  import { count, scopeLabel } from '../shell/scope';
  import ConfigMenu from './ConfigMenu.svelte';
  import { configMode, type ConfigMode } from './config-mode.svelte';
  import { splitProfileNotice } from './profiles';
  import SectionDetail from './SectionDetail.svelte';
  import SectionList from './SectionList.svelte';
  import { filterSections, groupSections } from './sections';

  /** The chosen section, falling back to the first one shown when nothing (or something stale) is chosen. */
  function pick(view: ConfigView) {
    const first = groupSections(view.sections)[0]?.sections[0];

    return view.sections.find((section) => section.id === viewState.configSection) ?? first;
  }

  function choose(id: string): void {
    viewState.configSection = id;
    viewState.configQuery = '';
    drawer.close();
  }

  // Config files change on disk under an open view (the agent edits them), so it re-reads quietly.
  $effect(() => autoRefresh(() => void configView.refresh({ quiet: true }), 5000));

  const view = $derived(configView.data?.harness === harness.active?.info.id ? configView.data : undefined);
  const files = $derived(view?.sections.length);
  const section = $derived(view ? pick(view) : undefined);

  // A stale choice (a file that is gone) falls back to the first file, and a search the palette set for the old one goes with it.
  $effect(() => {
    if (section === undefined || viewState.configSection === section.id) {
      return;
    }

    if (viewState.configSection !== null) {
      viewState.configQuery = '';
    }

    viewState.configSection = section.id;
  });

  // A missing or unreadable file has nothing to search, switch or fold; only revealing it still makes sense.
  const readable = $derived(section !== undefined && section.source.exists && !section.error);
  const matches = $derived(section && viewState.configQuery.trim() !== '' ? countMatches(section.entries, viewState.configQuery) : 0);

  let pane: SectionDetail | undefined = $state();
  let openCount = $state(0);

  const modes = $derived<StripOption<ConfigMode>[]>([
    { value: 'structured', label: t('config.mode.structured'), icon: LayoutGrid },
    { value: 'source', label: t('config.mode.source'), icon: Code },
  ]);

  function onSearchKey(event: KeyboardEvent): void {
    if (event.key === 'Enter' && configMode.value === 'source') {
      event.preventDefault();
      pane?.step(event.shiftKey ? -1 : 1);
    } else if (event.key === 'Escape' && viewState.configQuery !== '') {
      event.preventDefault();
      event.stopPropagation();
      viewState.configQuery = '';
    }
  }

  async function copyPath(path: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(path);
    } catch {
      toasts.push({ title: t('components.copy.failed'), description: t('components.copy.unavailable'), tone: 'warning' });

      return;
    }

    toasts.push({ title: t('components.copy.copied'), tone: 'success' });
  }

  const drawer = new PaneDrawer();

  // A search that leaves the chosen file out of the list moves the detail to the first file that is still in it.
  $effect(() => {
    const first = groupSections(filterSections(view?.sections ?? [], viewState.configQuery))[0]?.sections[0];

    if (section && first && !filterSections([section], viewState.configQuery).length) {
      viewState.configSection = first.id;
    }
  });
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title={t('config.title')} icon={SlidersHorizontal} context={scopeLabel(files === undefined ? null : count(files, 'file'))}>
    {#snippet actions()}
      {#if section}
        {#if readable}
          <div class="w-[340px] max-[900px]:w-[220px]">
            <SearchField
              bind:value={viewState.configQuery}
              label={t('config.search.label')}
              placeholder={t('config.search.placeholder')}
              shortcut="mod+f"
              status={matches > 0 ? tn('config.search.matches', matches) : undefined}
              onkeydown={onSearchKey}
            />
          </div>
        {/if}

        <DrawerToggle {drawer} label={t('config.drawer.label')} count={files} />

        {#if readable}
          <SegmentedControl options={modes} value={configMode.value} label={t('config.mode.label')} onchange={(mode) => configMode.set(mode)} />
        {/if}

        <ConfigMenu
          label={t('config.menu.label')}
          {readable}
          exists={section.source.exists}
          anyOpen={openCount > 0}
          ontoggleall={() => pane?.setAll(openCount === 0)}
          oncopypath={() => void copyPath(section.source.path)}
          onreveal={() => void revealPath(section.source.path)}
        />
      {/if}
    {/snippet}
  </ViewHeader>

  <div class="relative min-h-0 flex-1">
    <HarnessGate loader={configView} area="config">
      {#snippet children(view)}
        {@const section = pick(view)}
        {@const profiles = splitProfileNotice(view.notices)}

        {#if profiles.rest.length > 0}
          <div class="page-gutter border-b border-line py-3"><NoticeBanner notices={profiles.rest} /></div>
        {/if}

        <div class="min-h-0 flex-1">
          {#if section}
            <ListDetail label={t('config.resize')} scrollKey="config:detail" size={288} {drawer} drawerLabel={t('config.drawer.label')}>
              {#snippet list()}
                <SectionList sections={filterSections(view.sections, viewState.configQuery)} selectedId={section.id} searching={viewState.configQuery.trim() !== ''} onselect={choose} />
              {/snippet}
              {#snippet detail()}
                {#key section.id}
                  <SectionDetail bind:this={pane} bind:openCount {section} query={viewState.configQuery} mode={configMode.value} {profiles} />
                {/key}
              {/snippet}
            </ListDetail>
          {:else}
            <p class="p-8 text-center text-base text-fg-2" role="status">{t('config.empty')}</p>
          {/if}
        </div>
      {/snippet}
    </HarnessGate>
  </div>
</div>

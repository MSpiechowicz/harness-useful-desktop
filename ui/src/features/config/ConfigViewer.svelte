<script lang="ts">
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';

  import ListDetail from '$lib/components/ListDetail.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import RefreshButton from '$lib/components/RefreshButton.svelte';
  import type { ConfigView } from '$lib/ipc/bindings/ConfigView';
  import { harness } from '$lib/stores/harness.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { configView } from '$lib/stores/views.svelte';

  import HarnessGate from '../harness/HarnessGate.svelte';
  import { count, scopeLabel } from '../shell/scope';
  import SectionDetail from './SectionDetail.svelte';
  import SectionList from './SectionList.svelte';
  import { groupSections } from './sections';

  /** The chosen section, falling back to the first one shown when nothing (or something stale) is chosen. */
  function pick(view: ConfigView) {
    const first = groupSections(view.sections)[0]?.sections[0];

    return view.sections.find((section) => section.id === viewState.configSection) ?? first;
  }

  function choose(id: string): void {
    viewState.configSection = id;
    viewState.configQuery = '';
  }

  const files = $derived(configView.data?.harness === harness.active?.info.id ? configView.data?.sections.length : undefined);
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title="Configuration" icon={SlidersHorizontal} context={scopeLabel(files === undefined ? null : count(files, 'file'))}>
    {#snippet actions()}
      <RefreshButton label="Reload configuration" onclick={() => configView.refresh()} />
    {/snippet}
  </ViewHeader>

  <div class="relative min-h-0 flex-1">
    <HarnessGate loader={configView} noun="configuration">
      {#snippet children(view)}
        {@const section = pick(view)}

        {#if view.notices.length > 0}
          <div class="page-gutter pt-3"><NoticeBanner notices={view.notices} /></div>
        {/if}

        <div class="mt-3 min-h-0 flex-1 border-t border-line">
          {#if section}
            <ListDetail label="Resize file list" scrollKey="config:detail">
              {#snippet list()}
                <SectionList sections={view.sections} selectedId={section.id} onselect={choose} />
              {/snippet}
              {#snippet detail()}
                {#key section.id}
                  <SectionDetail {section} bind:query={viewState.configQuery} />
                {/key}
              {/snippet}
            </ListDetail>
          {:else}
            <p class="p-8 text-center text-base text-fg-2" role="status">No configuration files were found for this harness.</p>
          {/if}
        </div>
      {/snippet}
    </HarnessGate>
  </div>
</div>

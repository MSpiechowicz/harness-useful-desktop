<script lang="ts">
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import ListDetail from '$lib/components/ListDetail.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import type { ConfigView } from '$lib/ipc/bindings/ConfigView';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { configView } from '$lib/stores/views.svelte';

  import HarnessGate from '../harness/HarnessGate.svelte';
  import SectionDetail from './SectionDetail.svelte';
  import SectionList from './SectionList.svelte';
  import { groupSections } from './sections';

  let selectedId = $state<string | null>(null);

  /** The chosen section, falling back to the first one shown when nothing (or something stale) is chosen. */
  function pick(view: ConfigView) {
    const first = groupSections(view.sections)[0]?.sections[0];

    return view.sections.find((section) => section.id === selectedId) ?? first;
  }
</script>

<HarnessGate loader={configView} noun="configuration">
  {#snippet children(view)}
    {@const section = pick(view)}
    <header class="flex items-center gap-3 px-6 pt-5 pb-3">
      {#if harness.active}
        <HarnessBadge info={harness.active.info} />
      {/if}
      <div class="flex min-w-0 flex-1 flex-col">
        <h2 class="text-xl text-fg">Configuration</h2>
        <p class="truncate text-sm text-fg-2">
          {harness.active?.info.name} · {project.active ? project.active.name : 'no project open'}
        </p>
      </div>
      <IconButton icon={RotateCw} label="Reload configuration" side="bottom" onclick={() => configView.refresh()} />
    </header>

    {#if view.notices.length > 0}
      <div class="px-6 pb-3"><NoticeBanner notices={view.notices} /></div>
    {/if}

    <div class="min-h-0 flex-1 border-t border-line">
      {#if section}
        <ListDetail label="Resize file list">
          {#snippet list()}
            <SectionList sections={view.sections} selectedId={section.id} onselect={(id) => (selectedId = id)} />
          {/snippet}
          {#snippet detail()}
            {#key section.id}
              <SectionDetail {section} />
            {/key}
          {/snippet}
        </ListDetail>
      {:else}
        <p class="p-8 text-center text-base text-fg-2" role="status">No configuration files were found for this harness.</p>
      {/if}
    </div>
  {/snippet}
</HarnessGate>

<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
  import { cx } from '$lib/utils/cx';

  import { baseName, FORMAT_LABELS, groupSections } from './sections';

  interface Props {
    sections: ConfigSection[];
    selectedId: string;
    onselect: (id: string) => void;
  }

  let { sections, selectedId, onselect }: Props = $props();

  const groups = $derived(groupSections(sections));
</script>

<nav aria-label="Configuration files" class="flex flex-col gap-5 overflow-y-auto p-3">
  {#each groups as group (group.layer)}
    <section class="flex flex-col gap-1" aria-label="{group.title} layer">
      <h3 class="px-2 text-xs font-medium tracking-wider text-fg-3 uppercase">{group.title}</h3>

      {#each group.sections as section (section.id)}
        {@const selected = section.id === selectedId}
        <button
          type="button"
          aria-current={selected ? 'true' : undefined}
          onclick={() => onselect(section.id)}
          class={cx(
            'flex flex-col gap-1 rounded-md border px-3 py-2 text-left transition-colors duration-[var(--dur-fast)]',
            selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover',
          )}
        >
          <span class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-base font-medium text-fg">{section.title}</span>
            {#if section.source.profile && !section.title.includes(section.source.profile)}
              <Badge tone="info">{section.source.profile}</Badge>
            {/if}
          </span>

          <span class="flex items-center gap-2 text-xs text-fg-3">
            <span class="min-w-0 flex-1 truncate font-mono">{baseName(section.source.path)}</span>
            {#if section.error}
              <Badge tone="danger">Error</Badge>
            {:else if !section.source.exists}
              <Badge tone="neutral">Not found</Badge>
            {/if}
            <Badge>{FORMAT_LABELS[section.source.format]}</Badge>
          </span>
        </button>
      {/each}
    </section>
  {/each}
</nav>

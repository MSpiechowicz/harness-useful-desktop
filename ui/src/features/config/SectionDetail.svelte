<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import FileQuestion from '@lucide/svelte/icons/file-question-mark';
  import FolderSearch from '@lucide/svelte/icons/folder-search';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import ConfigTree from '$lib/components/ConfigTree.svelte';
  import { countMatches } from '$lib/components/config-tree';
  import SearchField from '$lib/components/SearchField.svelte';
  import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
  import { revealPath } from '$lib/ipc/actions';

  import { FORMAT_LABELS, LAYER_TITLES } from './sections';

  interface Props {
    section: ConfigSection;
  }

  let { section }: Props = $props();

  let query = $state('');

  const matches = $derived(countMatches(section.entries, query));
  const layer = $derived(section.source.profile ? `${LAYER_TITLES[section.source.layer]} · ${section.source.profile}` : LAYER_TITLES[section.source.layer]);
</script>

<article class="flex min-h-full flex-col gap-5 p-6" aria-labelledby="section-title">
  <header class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <h2 id="section-title" class="text-2xl text-fg">{section.title}</h2>
      <Badge tone="accent">{layer}</Badge>
      <Badge>{FORMAT_LABELS[section.source.format]}</Badge>
    </div>

    {#if section.description}
      <p class="text-base text-fg-2">{section.description}</p>
    {/if}

    <div class="flex items-center gap-3 rounded-md border border-line bg-surface-1 px-3 py-2">
      <code class="min-w-0 flex-1 truncate font-mono text-sm text-fg-2" title={section.source.path}>{section.source.path}</code>
      <Button size="sm" disabled={!section.source.exists} onclick={() => void revealPath(section.source.path)}>
        {#snippet leading()}<FolderSearch size={14} aria-hidden="true" />{/snippet}
        Reveal in file manager
      </Button>
    </div>
  </header>

  {#if section.error}
    <div class="flex items-start gap-3 rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
      <CircleAlert class="mt-0.5 shrink-0 text-danger" size={16} aria-hidden="true" />
      <div class="flex flex-col gap-1">
        <p class="font-medium">This file could not be read</p>
        <p class="text-fg-2">{section.error}</p>
      </div>
    </div>
  {:else if !section.source.exists}
    <div class="flex items-start gap-3 rounded-md border border-line bg-surface-1 p-4 text-sm" role="status">
      <FileQuestion class="mt-0.5 shrink-0 text-fg-3" size={16} aria-hidden="true" />
      <div class="flex flex-col gap-1">
        <p class="font-medium text-fg">No file here yet</p>
        <p class="text-fg-2">The harness has no settings at this path. Settings from this layer will appear once the file exists.</p>
      </div>
    </div>
  {:else}
    <div class="flex items-center gap-3">
      <SearchField bind:value={query} label="Filter keys and values" placeholder="Filter keys and values" class="max-w-sm flex-1" />
      <p class="text-sm text-fg-3" aria-live="polite">
        {#if query.trim() !== ''}
          {matches} {matches === 1 ? 'match' : 'matches'}
        {:else}
          {section.entries.length} top-level {section.entries.length === 1 ? 'entry' : 'entries'}
        {/if}
      </p>
    </div>

    <div class="rounded-lg border border-line bg-surface-1 p-2">
      <ConfigTree fields={section.entries} label="{section.title} settings" {query} />
    </div>
  {/if}
</article>

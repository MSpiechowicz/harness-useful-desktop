<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import FileQuestion from '@lucide/svelte/icons/file-question-mark';
  import FolderSearch from '@lucide/svelte/icons/folder-search';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import ConfigTree from '$lib/components/ConfigTree.svelte';
  import { countMatches } from '$lib/components/config-tree';
  import { ICON } from '$lib/components/icon';
  import SearchField from '$lib/components/SearchField.svelte';
  import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
  import { revealPath } from '$lib/ipc/actions';
  import { breakable } from '$lib/utils/format';

  import { baseName, FORMAT_LABELS, LAYER_TITLES } from './sections';

  interface Props {
    section: ConfigSection;
    /** The filter over keys and values; the command palette sets it to jump to a key. */
    query?: string;
  }

  let { section, query = $bindable('') }: Props = $props();

  const matches = $derived(countMatches(section.entries, query));
  const layer = $derived(section.source.profile ? `${LAYER_TITLES[section.source.layer]} · ${section.source.profile}` : LAYER_TITLES[section.source.layer]);
  const facts = $derived([
    { label: 'Layer', value: layer },
    { label: 'Format', value: FORMAT_LABELS[section.source.format] },
    { label: 'File', value: baseName(section.source.path) },
    { label: 'Entries', value: String(section.entries.length) },
  ]);
</script>

<article class="flex min-h-full flex-col gap-5 p-6" aria-labelledby="section-title">
  <header class="flex flex-col gap-2">
    <div class="flex flex-wrap items-center gap-2">
      <h2 id="section-title" class="text-2xl text-fg">{section.title}</h2>
      <Badge tone="accent">{layer}</Badge>
      <Badge>{FORMAT_LABELS[section.source.format]}</Badge>
    </div>

    {#if section.description}
      <p class="max-w-3xl text-base text-fg-2">{section.description}</p>
    {/if}
  </header>

  <!-- Wide panes put the file facts beside the tree instead of leaving that space empty. -->
  <div class="grid gap-5 @3xl:grid-cols-[minmax(0,1fr)_17rem] @3xl:items-start">
    <aside aria-label="Source file" class="flex flex-col gap-3 rounded-lg border border-line bg-surface-1 px-3 py-2 @3xl:sticky @3xl:top-0 @3xl:order-last @3xl:p-4">
      <div class="flex items-center gap-3 @3xl:flex-col @3xl:items-stretch @3xl:gap-3">
        <code class="min-w-0 flex-1 truncate font-mono text-sm text-fg-2 @3xl:whitespace-normal" title={section.source.path}>{breakable(section.source.path)}</code>
        <Button size="sm" disabled={!section.source.exists} onclick={() => void revealPath(section.source.path)}>
          {#snippet leading()}<FolderSearch size={ICON.sm} aria-hidden="true" />{/snippet}
          Reveal in file manager
        </Button>
      </div>

      <dl class="hidden grid-cols-[max-content_1fr] gap-x-4 gap-y-2 border-t border-line pt-3 text-sm @3xl:grid">
        {#each facts as fact (fact.label)}
          <dt class="text-fg-3">{fact.label}</dt>
          <dd class="min-w-0 truncate text-fg">{fact.value}</dd>
        {/each}
      </dl>

      <p class="hidden items-start gap-2 border-t border-line pt-3 text-xs text-fg-3 @3xl:flex">
        <ShieldCheck size={ICON.sm} class="mt-px shrink-0 text-success" aria-hidden="true" />
        Secrets are masked here and never leave the backend.
      </p>
    </aside>

    <div class="flex min-w-0 flex-col gap-5">
      {#if section.error}
        <div class="flex items-start gap-3 rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
          <CircleAlert class="mt-0.5 shrink-0 text-danger" size={ICON.sm} aria-hidden="true" />
          <div class="flex flex-col gap-1">
            <p class="font-medium">This file could not be read</p>
            <p class="text-fg-2">{section.error}</p>
          </div>
        </div>
      {:else if !section.source.exists}
        <div class="flex items-start gap-3 rounded-md border border-line bg-surface-1 p-4 text-sm" role="status">
          <FileQuestion class="mt-0.5 shrink-0 text-fg-3" size={ICON.sm} aria-hidden="true" />
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
    </div>
  </div>
</article>

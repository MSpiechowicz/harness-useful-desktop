<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import FileQuestion from '@lucide/svelte/icons/file-question-mark';
  import FileText from '@lucide/svelte/icons/file-text';
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
  import { cx } from '$lib/utils/cx';

  import { FORMAT_LABELS, LAYER_TITLES } from './sections';

  interface Props {
    section: ConfigSection;
    /** The filter over keys and values; the command palette sets it to jump to a key. */
    query?: string;
  }

  let { section, query = $bindable('') }: Props = $props();

  const matches = $derived(countMatches(section.entries, query));

  let tree: ConfigTree | undefined = $state();
  let openCount = $state(0);
  const layer = $derived(section.source.profile ? `${LAYER_TITLES[section.source.layer]} · ${section.source.profile}` : LAYER_TITLES[section.source.layer]);
</script>

<!-- One column: title, where the file lives, then the tree. Nothing floats beside the content. -->
<article class="flex min-h-full max-w-5xl flex-col gap-5 p-6" aria-labelledby="section-title">
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

  <!-- The source file, as one row: path on the left, the only action on the right. -->
  <div class="flex items-center gap-3 rounded-md border border-line bg-surface-1 py-1.5 pr-1.5 pl-3">
    <FileText size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
    <code class="min-w-0 flex-1 truncate font-mono text-sm text-fg-2" title={section.source.path}>{section.source.path}</code>
    <Button size="sm" variant="ghost" disabled={!section.source.exists} onclick={() => void revealPath(section.source.path)}>
      {#snippet leading()}<FolderSearch size={ICON.sm} aria-hidden="true" />{/snippet}
      Reveal in file manager
    </Button>
  </div>

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

      <!-- One toggle for the whole tree: collapse while anything is open, otherwise expand everything. One chevron that turns, like the rows'. -->
      <Button size="sm" variant="ghost" class="ml-auto" onclick={() => tree?.setAll(openCount === 0)}>
        {#snippet leading()}<ChevronDown size={ICON.sm} class={cx('transition-transform duration-[var(--dur-fast)]', openCount > 0 && 'rotate-180')} aria-hidden="true" />{/snippet}
        {openCount > 0 ? 'Collapse all' : 'Expand all'}
      </Button>
    </div>

    <div class="rounded-lg border border-line bg-surface-1 p-2">
      <ConfigTree bind:this={tree} bind:openCount fields={section.entries} label="{section.title} settings" {query} />
    </div>

    <p class="flex items-center gap-2 text-xs text-fg-3">
      <ShieldCheck size={ICON.sm} class="shrink-0 text-success" aria-hidden="true" />
      Secrets are masked here and never leave the backend.
    </p>
  {/if}
</article>

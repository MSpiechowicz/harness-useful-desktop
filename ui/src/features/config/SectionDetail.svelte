<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import FileQuestion from '@lucide/svelte/icons/file-question-mark';

  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';

  import type { ConfigMode } from './config-mode.svelte';
  import SectionAside from './SectionAside.svelte';
  import SourceView from './SourceView.svelte';
  import StructuredView from './StructuredView.svelte';

  interface Props {
    section: ConfigSection;
    /** The search over keys and values; the command palette sets it to jump to a key. */
    query?: string;
    mode: ConfigMode;
    profiles?: { names: string[]; hint: string | null };
    /** Out: how many foldable rows of the content are open, so the top bar's menu can collapse or expand them. */
    openCount?: number;
  }

  let { section, query = '', mode, profiles = { names: [], hint: null }, openCount = $bindable(0) }: Props = $props();

  let structured: StructuredView | undefined = $state();
  let source: SourceView | undefined = $state();

  /** Opens or closes every foldable row of whichever view is showing. */
  export function setAll(open: boolean): void {
    if (mode === 'source') {
      source?.setAll(open);
    } else {
      structured?.setAll(open);
    }
  }

  /** Moves to the next or previous match in the source; the structured view filters instead of stepping. */
  export function step(direction: 1 | -1): void {
    source?.step(direction);
  }
</script>

<!-- Title and content on the left, the file's facts in a column of cards on the right (below the content when narrow). -->
<article class="flex min-h-full flex-col @[760px]:h-full @[760px]:flex-row" aria-labelledby="section-title">
  <div class="flex min-w-0 flex-1 flex-col @[760px]:min-h-0">
    <header class="shrink-0 border-b border-line px-5 pt-4 pb-3">
      <h2 id="section-title" class="truncate text-lg leading-6 font-semibold text-fg">{section.title}</h2>
      <p class="mt-0.5 truncate text-xs text-fg-3" title={section.source.path}>
        {#if section.description}{section.description} · {/if}<code class="font-mono">{section.source.path}</code>
      </p>
    </header>

    <div class="min-h-0 flex-1 px-5 pt-3 pb-6 @[760px]:overflow-y-auto">
      {#if section.error}
        <div class="mt-3 flex items-start gap-3 rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
          <CircleAlert class="mt-0.5 shrink-0 text-danger" size={ICON.sm} aria-hidden="true" />
          <div class="flex flex-col gap-1">
            <p class="font-medium">{t('config.detail.readError')}</p>
            <p class="text-fg-2">{section.error}</p>
          </div>
        </div>
      {:else if !section.source.exists}
        <div class="mt-3 flex items-start gap-3 rounded-md border border-line bg-surface-1 p-4 text-sm" role="status">
          <FileQuestion class="mt-0.5 shrink-0 text-fg-3" size={ICON.sm} aria-hidden="true" />
          <div class="flex flex-col gap-1">
            <p class="font-medium text-fg">{t('config.detail.missingTitle')}</p>
            <p class="text-fg-2">{t('config.detail.missingBody')}</p>
          </div>
        </div>
      {:else if mode === 'source'}
        <SourceView bind:this={source} bind:openCount fields={section.entries} {query} format={section.source.format} label={t('config.detail.treeLabel', { title: section.title })} />
      {:else}
        <StructuredView bind:this={structured} bind:openCount fields={section.entries} {query} format={section.source.format} label={t('config.detail.treeLabel', { title: section.title })} />
      {/if}
    </div>
  </div>

  <SectionAside {section} {profiles} class="shrink-0 border-t border-line @[760px]:w-[280px] @[760px]:overflow-y-auto @[760px]:border-t-0 @[760px]:border-l" />
</article>

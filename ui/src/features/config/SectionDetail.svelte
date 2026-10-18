<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import FileQuestion from '@lucide/svelte/icons/file-question-mark';

  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
  import { cx } from '$lib/utils/cx';

  import type { ConfigMode } from './config-mode.svelte';
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
    /** The content takes the height of the centre and scrolls itself (the source) instead of flowing in the page. */
    fill?: boolean;
  }

  let { section, query = '', mode, profiles = { names: [], hint: null }, openCount = $bindable(0), fill = false }: Props = $props();

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

<!-- The file's content. The title is the header of the view (`WorkspaceLayout` puts it above this). In the page the content flows and
     the page scrolls. With `fill` the content scrolls itself, and the article keeps the gutter and rhythm the page would have given it. -->
<article class={cx('flex flex-col', fill && 'page-gutter size-full min-h-0 py-6')} aria-labelledby="section-title">
  <div class={cx(fill && 'min-h-0 flex-1')}>
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
</article>

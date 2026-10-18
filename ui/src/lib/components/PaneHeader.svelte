<script lang="ts">
  import X from '@lucide/svelte/icons/x';
  import type { Snippet } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import IconButton from './IconButton.svelte';

  interface Props {
    title: string;
    /** The `id` of the title, for a region that is named by it (`aria-labelledby`). */
    titleId?: string;
    /** The whole of a shortened title (a file's path): its tooltip and its accessible name. */
    label?: string;
    /** The title is a file name or a path: monospace, a size smaller. */
    mono?: boolean;
    /** The line under the title: what the item is. Longer than the column, it is cut with an ellipsis before the actions. */
    subtitle?: Snippet;
    /** The full subtitle, for the tooltip of a cut one. */
    subtitleHint?: string;
    /** Small things on the title line after the title, such as a status badge. */
    meta?: Snippet;
    /** Controls at the end of the title line, before the close button. */
    actions?: Snippet;
    /** Shows a close button at the end of the title line. */
    onclose?: () => void;
    /** `page` is the gutter of the page, for the header of a centre. `panel` is the padding of a side panel. */
    inset?: 'page' | 'panel';
  }

  let { title, titleId, label, mono = false, subtitle, subtitleHint, meta, actions, onclose, inset = 'page' }: Props = $props();
</script>

<!-- The header band of a pane: the same height, line and type in every pane of a view, so the lines under the headers of the
     list, the centre and the details are one line. Two columns: the text, and the controls on the title line, so neither the
     title nor the subtitle runs under them. The title keeps a minimum and the controls give way first (a control that must not shrink says so itself). The controls' equal negative margin lets a 36px button overhang a line as high as
     the glyphs of the title: centring puts the button's middle on the middle of the letters without making the row taller. -->
<header class={cx('pane-header grid grid-cols-[minmax(var(--pane-title-min),1fr)_minmax(0,auto)] content-center items-center gap-y-[var(--pane-header-gap)] border-b border-panel-edge', inset === 'page' ? 'page-gutter' : 'panel-header-inset')}>
  <div class="flex min-w-0 items-center gap-3">
    <h2 id={titleId} class={cx('pane-text min-w-0 truncate text-lg leading-6 font-semibold text-fg', mono && 'font-mono text-base')} title={label ?? title} aria-label={label}>{title}</h2>
    {@render meta?.()}
  </div>

  {#if actions || onclose}
    <div class="ml-2 my-[calc(var(--control-h)/-2)] flex min-w-0 items-center gap-1 empty:hidden">
      {@render actions?.()}
      {#if onclose}
        <IconButton icon={X} label={t('components.panel.close')} class="shrink-0" onclick={onclose} />
      {/if}
    </div>
  {/if}

  {#if subtitle}
    <p class="pane-subtext col-start-1 truncate text-xs text-fg-3" title={subtitleHint}>{@render subtitle()}</p>
  {/if}
</header>

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
    /** Takes the place of the title and of the line under it while the title is edited in place: a field as tall as a control, centred in the band. */
    titleField?: Snippet;
    /** Small things on the title line after the title, such as a status badge. */
    meta?: Snippet;
    /** A state row under the title in place of the subtitle: a status badge with its grey line, 12px under the title. The band grows to hold it, with room above the title, instead of squeezing both into its fixed height. */
    status?: Snippet;
    /** Controls at the end of the title line, before the close button. */
    actions?: Snippet;
    /** Shows a close button at the end of the title line. */
    onclose?: () => void;
    /** `page` is the gutter of the page, for the header of a centre. `panel` is the padding of a side panel. */
    inset?: 'page' | 'panel';
  }

  let { title, titleId, label, mono = false, subtitle, subtitleHint, titleField, meta, status, actions, onclose, inset = 'page' }: Props = $props();
</script>

<!-- The header band of a pane: the same height, line and type in every pane of a view, so the lines under the headers of the
     list, the centre and the details are one line. Two columns: the text, and the controls on the title line, so neither the
     title nor the subtitle runs under them. The title keeps a minimum and the controls give way first (a control that must not shrink says so itself). The controls' equal negative margin lets a 36px button overhang a line as high as
     the glyphs of the title: centring puts the button's middle on the middle of the letters without making the row taller. -->
<header class={cx('pane-header grid grid-cols-[minmax(var(--pane-title-min),1fr)_minmax(0,auto)] content-center items-center gap-y-[var(--pane-header-gap)] border-b border-panel-edge', inset === 'page' ? 'page-gutter' : 'panel-header-inset', status && 'h-auto! min-h-(--pane-header-h) py-4')}>
  <div class="flex min-w-0 items-center gap-3">
    {#if titleField && status}
      <!-- With a state row under it, the field lies over the title's own line (kept, unseen, for its height), and the row under it
           is hidden in its place: the band keeps its height while the title is edited. -->
      <div class="relative min-w-0 flex-1">
        <span aria-hidden="true" class={cx('pane-text invisible block min-w-0 truncate text-lg leading-6 font-semibold', mono && 'font-mono text-base')}>{title}</span>
        <div class="absolute inset-x-0 top-1/2 -translate-y-1/2">{@render titleField()}</div>
      </div>
    {:else if titleField}
      {@render titleField()}
    {:else}
      <h2 id={titleId} class={cx('pane-text min-w-0 truncate text-lg leading-6 font-semibold text-fg', mono && 'font-mono text-base')} title={label ?? title} aria-label={label}>{title}</h2>
    {/if}
    {@render meta?.()}
  </div>

  {#if actions || onclose}
    <div class={cx('my-[calc(var(--control-h)/-2)] flex min-w-0 items-center gap-1 empty:hidden', titleField ? 'ml-4' : 'ml-2')}>
      {@render actions?.()}
      {#if onclose}
        <IconButton icon={X} label={t('components.panel.close')} class="shrink-0" onclick={onclose} />
      {/if}
    </div>
  {/if}

  {#if status}
    <!-- The band's gap plus 6px puts the badge 12px under the title's baseline. It is not a line of text, so it is not trimmed like the subtitle. -->
    <!-- While the title is edited the row is hidden but keeps its room, so the band does not change height. -->
    <div class={cx('col-start-1 mt-1.5 flex min-w-0 items-center gap-3', titleField && 'invisible')} aria-hidden={titleField ? 'true' : undefined}>{@render status()}</div>
  {:else if subtitle && !titleField}
    <p class="pane-subtext col-start-1 truncate text-xs text-fg-3" title={subtitleHint}>{@render subtitle()}</p>
  {/if}
</header>

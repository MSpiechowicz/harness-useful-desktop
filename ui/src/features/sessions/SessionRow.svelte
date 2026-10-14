<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import GitFork from '@lucide/svelte/icons/git-fork';
  import Layers from '@lucide/svelte/icons/layers';

  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';
  import { compactAge } from '$lib/utils/format';

  import { sessionTitle } from './session-format';
  import SessionMenu from './SessionMenu.svelte';

  interface Props {
    summary: SessionSummary;
    selected: boolean;
    /** An empty copy shown under its session: indented, otherwise the same row. */
    copy?: boolean;
    /** The last copy under its session, where the tree line ends. */
    last?: boolean;
    /** How many empty copies are folded under this session. */
    copies?: number;
    expanded?: boolean;
    height: number;
    onselect: () => void;
    ontoggle?: () => void;
    ondelete: () => void;
  }

  let { summary, selected, copy = false, last = false, copies = 0, expanded = false, height, onselect, ontoggle, ondelete }: Props = $props();

  let menuOpen = $state(false);

  const title = $derived(sessionTitle(summary));
  const info = $derived(harness.get(summary.ref.harness)?.info);
</script>

<!-- One line, like a file in the Changes list: the harness tile, the title and how old the session is. Everything else
     about it is in the details beside the transcript. The button that selects the row and the menu are siblings; the text
     sits above the button and lets clicks through to it. Copies hang on a tree line that starts under their session's
     title and turns into each copy. The row with an open menu sits above its neighbours, or the next row would cover
     the menu. -->
<div class={cx('relative px-3 py-[2px]', copy && 'pl-[40px]', expanded && 'pb-[calc(0.125rem+var(--tree-gap))]', menuOpen && 'z-10')} style:height="{height}px">
  <!-- Each segment is drawn once: the lines are translucent, and an overlap would show as a brighter dot. The elbow
       points at the copy's title (its middle is 20px into the 40px row). -->
  {#if expanded}<span aria-hidden="true" class="absolute bottom-0 left-[22px] h-[calc(0.125rem+var(--tree-gap))] w-px bg-line-strong"></span>{/if}
  {#if copy}
    <span aria-hidden="true" class={cx('absolute top-0 left-[22px] w-px bg-line-strong', last ? 'h-[20px]' : 'bottom-0')}></span>
    <span aria-hidden="true" class="absolute top-[19px] left-[23px] h-px w-[17px] bg-line-strong"></span>
  {/if}

  <div class={cx('group/row relative size-full rounded-md border transition-colors duration-[var(--dur-fast)]', selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover')}>
    <button
      type="button"
      data-session={summary.ref.id}
      aria-label={title}
      aria-current={selected ? 'true' : undefined}
      class="absolute inset-0 rounded-md focus-visible:outline-offset-[-1px]"
      onclick={onselect}
    ></button>

    <div class="pointer-events-none relative flex size-full items-center gap-2.5 pr-2 pl-2.5">
      {#if info}<HarnessBadge {info} size="xs" />{/if}
      <span class="min-w-0 flex-1 truncate text-sm text-fg" aria-hidden="true">{title}</span>
      {#if summary.parentSession && !summary.emptyCopy}<GitFork size={14} class="shrink-0 text-fg-3" aria-label={t('sessions.list.forked')} />{/if}
      {#if copies > 0}
        <button
          type="button"
          aria-expanded={expanded}
          class="pointer-events-auto inline-flex shrink-0 items-center gap-1 rounded-full bg-surface-3 py-px pr-2 pl-1.5 text-[11px] leading-5 text-fg-2 transition-colors hover:bg-surface-4 hover:text-fg"
          onclick={ontoggle}
        >
          <ChevronRight size={12} class={cx('transition-transform duration-[var(--dur-fast)]', expanded && 'rotate-90')} aria-hidden="true" />
          <Layers size={12} aria-hidden="true" />
          {tn('sessions.list.copies', copies)}
        </button>
      {/if}
      <span
        class={cx('tabular w-8 shrink-0 text-right text-xs text-fg-3 transition-opacity duration-[var(--dur-fast)] group-focus-within/row:opacity-0 group-hover/row:opacity-0', (selected || menuOpen) && 'opacity-0')}
        title={summary.updatedAt}
      >{compactAge(summary.updatedAt)}</span>
    </div>

    <div
      class={cx(
        'absolute top-1/2 right-1 -translate-y-1/2 opacity-0 transition-opacity duration-[var(--dur-fast)] focus-within:opacity-100 group-hover/row:opacity-100',
        (selected || menuOpen) && 'opacity-100',
      )}
    >
      <SessionMenu label={t('sessions.list.menu')} bind:open={menuOpen} {ondelete} />
    </div>
  </div>
</div>

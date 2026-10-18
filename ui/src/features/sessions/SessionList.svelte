<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Layers from '@lucide/svelte/icons/layers';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { openChatOf } from '$lib/stores/session-actions';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  import { matchesQuery } from './session-format';
  import type { SessionGroups } from './session-groups';
  import { folded, SECTION_IDS, SECTION_LABELS, sectionOf, type SectionId } from './session-sections';
  import SessionRow from './SessionRow.svelte';

  interface Props {
    groups: SessionGroups;
    selectedId: string | null;
    onselect: (summary: SessionSummary) => void;
    ondelete: (summary: SessionSummary) => void;
    oncleanup: (copies: SessionSummary[]) => void;
  }

  let { groups, selectedId, onselect, ondelete, oncleanup }: Props = $props();

  const ROW_PX = 40;
  /** A copy row is shorter: it hangs off its session and needs no room between cards. */
  const COPY_PX = 36;
  const HEADING_PX = 48;
  /** Extra room under a session whose copies are open, so the first copy sits as far below it as the copies sit apart. */
  const TREE_GAP_PX = 5;

  interface Heading {
    kind: 'heading';
    id: SectionId;
    count: number;
    open: boolean;
  }

  interface Entry {
    kind: 'session';
    summary: SessionSummary;
    /** An empty copy listed under its session. */
    copy: boolean;
    /** The last copy under its session. */
    last: boolean;
    copies: number;
    /** Whether the copies of this session are listed below it. */
    open: boolean;
  }

  let expanded = $state.raw<ReadonlySet<string>>(new Set());
  let container: HTMLElement | undefined = $state();

  // A copy a chat keeps open is not clutter: it stays out of the clean-up.
  const cleanable = $derived(groups.emptyCopies.filter((copy) => !openChatOf(agentSessions, copy.ref)));
  const shown = $derived(groups.rows.filter((summary) => matchesQuery(summary, viewState.sessionsQuery)));

  /** A session shows its empty copies when expanded, or when one of them is the selected session. */
  function isOpen(rootId: string): boolean {
    const copies = groups.copiesByRoot.get(rootId) ?? [];

    return expanded.has(rootId) || copies.some((copy) => copy.ref.id === selectedId);
  }

  /** While searching, every heading stays open so no match hides behind a fold. */
  const searching = $derived(viewState.sessionsQuery.trim() !== '');

  const entries = $derived.by((): (Heading | Entry)[] => {
    const now = Date.now();

    return SECTION_IDS.flatMap((id) => {
      const inside = shown.filter((summary) => sectionOf(summary.updatedAt, now) === id);
      if (inside.length === 0) {
        return [];
      }

      const open = searching || !folded.has(id);
      const heading: Heading = { kind: 'heading', id, count: inside.length, open };
      if (!open) {
        return [heading];
      }

      const rows = inside.flatMap((summary): Entry[] => {
        const copies = groups.copiesByRoot.get(summary.ref.id) ?? [];
        const opened = isOpen(summary.ref.id);

        const root: Entry = { kind: 'session', summary, copy: false, last: false, copies: copies.length, open: opened };
        const listed = opened ? copies.map((copy, index): Entry => ({ kind: 'session', summary: copy, copy: true, last: index === copies.length - 1, copies: 0, open: false })) : [];

        return [root, ...listed];
      });

      return [heading, ...rows];
    });
  });

  /** The sessions a person can reach, top to bottom, for arrow-key navigation. */
  const reachable = $derived(entries.filter((entry): entry is Entry => entry.kind === 'session'));

  function toggle(id: string): void {
    const next = new Set(expanded);

    if (!next.delete(id)) {
      next.add(id);
    }

    expanded = next;
  }

  function onKeydown(event: KeyboardEvent): void {
    const steps: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    if (!(event.key in steps) || reachable.length === 0) {
      return;
    }

    // Only a row's own button moves the selection; a row's menu and the copies chip keep their keys.
    if (!(event.target instanceof HTMLElement) || !event.target.matches('[data-session]')) {
      return;
    }

    event.preventDefault();
    const current = reachable.findIndex((entry) => entry.summary.ref.id === selectedId);
    const next = reachable[Math.min(reachable.length - 1, Math.max(0, current + steps[event.key]))].summary;

    onselect(next);
    container?.querySelector<HTMLElement>(`[data-session="${CSS.escape(next.ref.id)}"]`)?.focus({ preventScroll: false });
  }
</script>

<div class="flex min-h-0 flex-1 flex-col pt-1 pb-3">
  {#if cleanable.length > 0}
    <div class="mx-[var(--panel-row-inset)] mb-2 flex items-center gap-2.5 rounded-lg border border-accent/25 bg-accent-wash panel-row py-2 text-sm text-fg-2">
      <Layers size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />
      <span class="min-w-0 flex-1">{tn('sessions.cleanup.banner', cleanable.length)}</span>
      <Button onclick={() => oncleanup(cleanable)}>{t('sessions.cleanup.action')}</Button>
    </div>
  {/if}

  {#if entries.length === 0}
    <p class="p-6 text-center text-sm text-fg-3" role="status">{viewState.sessionsQuery === '' ? t('sessions.list.empty') : t('sessions.list.noMatch')}</p>
  {:else}
    <div bind:this={container} class="min-h-0 flex-1" role="presentation" style:--tree-gap="{TREE_GAP_PX}px" onkeydown={onKeydown}>
      <VirtualList items={entries} estimate={ROW_PX} variable overscan={8} label={t('sessions.list.label')} key={(entry) => (entry.kind === 'heading' ? `heading:${entry.id}` : entry.summary.ref.id)} class="size-full">
        {#snippet row(entry)}
          {#if entry.kind === 'heading'}
            <div class="panel-inset pt-[14px] pb-[6px]" style:height="{HEADING_PX}px">
              <button
                type="button"
                aria-expanded={entry.open}
                disabled={searching}
                class="group/heading flex h-[28px] w-full items-center gap-[10px] rounded-sm panel-heading text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px] disabled:hover:bg-transparent disabled:hover:text-fg-3"
                onclick={() => folded.toggle(entry.id)}
              >
                <span class="grid size-[16px] shrink-0 place-items-center"><ChevronRight size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', entry.open && 'rotate-90')} /></span>
                <span class="flex-1 truncate">{t(SECTION_LABELS[entry.id])}</span>
                <span class="tabular">{formatCount(entry.count)}</span>
              </button>
            </div>
          {:else}
            <SessionRow
              summary={entry.summary}
              selected={entry.summary.ref.id === selectedId}
              copy={entry.copy}
              last={entry.last}
              copies={entry.copies}
              expanded={entry.open}
              height={entry.copy ? COPY_PX : entry.open ? ROW_PX + TREE_GAP_PX : ROW_PX}
              onselect={() => onselect(entry.summary)}
              ontoggle={() => toggle(entry.summary.ref.id)}
              ondelete={() => ondelete(entry.summary)}
            />
          {/if}
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

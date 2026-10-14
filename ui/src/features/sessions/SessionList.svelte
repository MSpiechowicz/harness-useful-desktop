<script lang="ts">
  import Layers from '@lucide/svelte/icons/layers';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import SearchField from '$lib/components/SearchField.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { openChatOf } from '$lib/stores/session-actions';

  import { matchesQuery } from './session-format';
  import type { SessionGroups } from './session-groups';
  import SessionRow from './SessionRow.svelte';

  interface Props {
    groups: SessionGroups;
    selectedId: string | null;
    onselect: (summary: SessionSummary) => void;
    ondelete: (summary: SessionSummary) => void;
    oncleanup: (copies: SessionSummary[]) => void;
  }

  let { groups, selectedId, onselect, ondelete, oncleanup }: Props = $props();

  const ROW_PX = 68;
  /** A copy row is shorter: it hangs off its session and needs no room between cards. */
  const COPY_PX = 52;
  /** Extra room under a session whose copies are open, so the first copy sits as far below it as the copies sit apart. */
  const TREE_GAP_PX = 5;

  interface Entry {
    summary: SessionSummary;
    /** An empty copy listed under its session. */
    copy: boolean;
    /** The last copy under its session. */
    last: boolean;
    copies: number;
    /** Whether the copies of this session are listed below it. */
    open: boolean;
  }

  let query = $state('');
  let expanded = $state.raw<ReadonlySet<string>>(new Set());
  let container: HTMLElement | undefined = $state();

  // A copy a chat keeps open is not clutter: it stays out of the clean-up.
  const cleanable = $derived(groups.emptyCopies.filter((copy) => !openChatOf(agentSessions, copy.ref)));
  const shown = $derived(groups.rows.filter((summary) => matchesQuery(summary, query)));

  /** A session shows its empty copies when expanded, or when one of them is the selected session. */
  function isOpen(rootId: string): boolean {
    const copies = groups.copiesByRoot.get(rootId) ?? [];

    return expanded.has(rootId) || copies.some((copy) => copy.ref.id === selectedId);
  }

  const entries = $derived(
    shown.flatMap((summary): Entry[] => {
      const copies = groups.copiesByRoot.get(summary.ref.id) ?? [];
      const open = isOpen(summary.ref.id);

      const root: Entry = { summary, copy: false, last: false, copies: copies.length, open };
      const listed = open ? copies.map((copy, index): Entry => ({ summary: copy, copy: true, last: index === copies.length - 1, copies: 0, open: false })) : [];

      return [root, ...listed];
    }),
  );

  function toggle(id: string): void {
    const next = new Set(expanded);

    if (!next.delete(id)) {
      next.add(id);
    }

    expanded = next;
  }

  function onKeydown(event: KeyboardEvent): void {
    const steps: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    if (!(event.key in steps) || entries.length === 0) {
      return;
    }

    // Only a row's own button moves the selection; a row's menu and the copies chip keep their keys.
    if (!(event.target instanceof HTMLElement) || !event.target.matches('[data-session]')) {
      return;
    }

    event.preventDefault();
    const current = entries.findIndex((entry) => entry.summary.ref.id === selectedId);
    const next = entries[Math.min(entries.length - 1, Math.max(0, current + steps[event.key]))].summary;

    onselect(next);
    container?.querySelector<HTMLElement>(`[data-session="${CSS.escape(next.ref.id)}"]`)?.focus({ preventScroll: false });
  }
</script>

<div class="flex min-h-0 flex-1 flex-col pb-3">
  <div class="px-3 pt-3 pb-2">
    <SearchField bind:value={query} label={t('sessions.list.search')} placeholder={t('sessions.list.searchPlaceholder')} />
  </div>

  {#if cleanable.length > 0}
    <div class="mx-3 mb-2 flex items-center gap-2.5 rounded-lg border border-accent/25 bg-accent-wash py-2 pr-2 pl-3 text-sm text-fg-2">
      <Layers size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />
      <span class="min-w-0 flex-1">{tn('sessions.cleanup.banner', cleanable.length)}</span>
      <Button onclick={() => oncleanup(cleanable)}>{t('sessions.cleanup.action')}</Button>
    </div>
  {/if}

  {#if entries.length === 0}
    <p class="p-6 text-center text-sm text-fg-3" role="status">{query === '' ? t('sessions.list.empty') : t('sessions.list.noMatch')}</p>
  {:else}
    <div bind:this={container} class="min-h-0 flex-1" role="presentation" style:--tree-gap="{TREE_GAP_PX}px" onkeydown={onKeydown}>
      <VirtualList items={entries} estimate={ROW_PX} variable overscan={8} label={t('sessions.list.label')} key={(entry) => entry.summary.ref.id} class="size-full">
        {#snippet row(entry)}
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
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

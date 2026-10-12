<script lang="ts">
  import GitFork from '@lucide/svelte/icons/git-fork';

  import SearchField from '$lib/components/SearchField.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { cx } from '$lib/utils/cx';
  import { formatBytes, relativeTime } from '$lib/utils/format';

  import { matchesQuery, sessionTitle } from './session-format';

  interface Props {
    sessions: SessionSummary[];
    selectedId: string | null;
    onselect: (summary: SessionSummary) => void;
  }

  let { sessions, selectedId, onselect }: Props = $props();

  const ROW_PX = 68;

  let query = $state('');
  let container: HTMLElement | undefined = $state();

  const shown = $derived(sessions.filter((summary) => matchesQuery(summary, query)));

  function onKeydown(event: KeyboardEvent): void {
    const steps: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    if (!(event.key in steps) || shown.length === 0) {
      return;
    }

    event.preventDefault();
    const current = shown.findIndex((summary) => summary.ref.id === selectedId);
    const next = shown[Math.min(shown.length - 1, Math.max(0, current + steps[event.key]))];

    onselect(next);
    container?.querySelector<HTMLElement>(`[data-session="${CSS.escape(next.ref.id)}"]`)?.focus({ preventScroll: false });
  }
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <div class="px-3 pt-3 pb-2">
    <SearchField bind:value={query} label={t('sessions.list.search')} placeholder={t('sessions.list.searchPlaceholder')} />
  </div>

  {#if shown.length === 0}
    <p class="p-6 text-center text-sm text-fg-3" role="status">{query === '' ? t('sessions.list.empty') : t('sessions.list.noMatch')}</p>
  {:else}
    <div bind:this={container} class="min-h-0 flex-1" role="presentation" onkeydown={onKeydown}>
      <VirtualList items={shown} estimate={ROW_PX} overscan={8} label={t('sessions.list.label')} key={(summary) => summary.ref.id} class="size-full">
        {#snippet row(summary)}
          {@const selected = summary.ref.id === selectedId}
          <!-- The same row as the plugin lists: a rounded card inset from the edges, outlined when selected. -->
          <div class="px-2 py-0.5" style:height="{ROW_PX}px">
            <button
              type="button"
              data-session={summary.ref.id}
              aria-current={selected ? 'true' : undefined}
              class={cx(
                'flex size-full flex-col justify-center gap-0.5 rounded-md border px-3 text-left transition-colors duration-[var(--dur-fast)] focus-visible:outline-offset-[-1px]',
                selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
              )}
              onclick={() => onselect(summary)}
            >
              <span class="truncate text-base font-medium text-fg">{sessionTitle(summary)}</span>
              <span class="tabular flex items-center gap-1.5 truncate text-xs text-fg-3">
                <span>{relativeTime(summary.updatedAt)}</span>
                {#if summary.model}<span class="truncate">· {summary.model}</span>{/if}
                <span class="shrink-0">· {formatBytes(summary.sizeBytes)}</span>
                {#if summary.parentSession}<GitFork size={16} class="shrink-0" aria-label={t('sessions.list.forked')} />{/if}
              </span>
            </button>
          </div>
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

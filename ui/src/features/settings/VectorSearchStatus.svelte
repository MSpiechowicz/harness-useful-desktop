<script lang="ts">
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { EmbedderPhase } from '$lib/ipc/bindings/EmbedderPhase';
  import { WORKING_PHASES, type EmbedderStateView } from '$lib/stores/embedder.svelte';
  import { cx } from '$lib/utils/cx';

  import type { EmbedderStatusCopy } from './embedder-status';

  interface Props {
    state: EmbedderStateView;
    /** What the status says, in the words of the Harness family member that owns the embedder. */
    copy: EmbedderStatusCopy;
  }

  let { state, copy }: Props = $props();

  const key = $derived(copy.phases[state.phase]);
  /** The phases that work through something: a bar shows how far, or sweeps while that is not known. */
  const busy = $derived((WORKING_PHASES as readonly EmbedderPhase[]).includes(state.phase));
  const fraction = $derived(typeof state.progress === 'number' ? Math.min(1, Math.max(0, state.progress)) : null);
  const failed = $derived(state.phase === 'failed');
  const ready = $derived(state.phase === 'ready');

  /** The dot says the state at a glance: live while it serves, accent while it works, danger when it stopped, neutral otherwise. */
  const dot = $derived(ready ? 'live-dot text-success' : busy ? 'live-dot text-accent' : failed ? 'size-2 rounded-full bg-current text-danger' : 'size-2 rounded-full bg-current text-fg-3');
</script>

{#if key}
  <div class="flex flex-col gap-2 pb-4" role="status">
    <div class="flex items-center justify-between gap-3 text-sm">
      <span class="flex min-w-0 items-center gap-2.5">
        <span aria-hidden="true" class="grid size-4 shrink-0 place-items-center"><span data-testid="vector-status-dot" class={dot}></span></span>
        <span class={cx('font-medium', failed ? 'text-danger' : 'text-fg')}>{t(key)}</span>
        {#if ready}
          <span class="min-w-0 text-fg-3">{t(copy.readyDetail)}</span>
        {/if}
      </span>
      {#if busy && fraction !== null}
        <span class="tabular text-fg-3">{t(copy.percent, { percent: Math.round(fraction * 100) })}</span>
      {/if}
    </div>

    {#if busy}
      <ProgressBar label={t(copy.progressLabel)} {fraction} />
    {:else if failed && state.error}
      <p class="text-sm text-fg-3">{state.error}</p>
    {/if}
  </div>
{/if}

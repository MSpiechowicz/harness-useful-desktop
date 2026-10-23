<script lang="ts">
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { EmbedderPhase } from '$lib/ipc/bindings/EmbedderPhase';
  import type { MemoryEmbedderState } from '$lib/ipc/bindings/MemoryEmbedderState';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import { cx } from '$lib/utils/cx';

  interface Props {
    state: MemoryEmbedderState;
  }

  let { state }: Props = $props();

  /** What the embedder says it does, per phase. `off` says nothing: the switch does. */
  const LABELS: Partial<Record<EmbedderPhase, MessageKey>> = {
    starting: 'settings.memory.vector.status.starting',
    downloading: 'settings.memory.vector.status.downloading',
    verifying: 'settings.memory.vector.status.verifying',
    loading: 'settings.memory.vector.status.loading',
    backfilling: 'settings.memory.vector.status.backfilling',
    updating: 'settings.memory.vector.status.updating',
    ready: 'settings.memory.vector.status.ready',
    elsewhere: 'settings.memory.vector.status.elsewhere',
    failed: 'settings.memory.vector.status.failed',
  };

  /** The phases that work through something: a bar shows how far, or sweeps while that is not known. */
  const BUSY: readonly EmbedderPhase[] = ['starting', 'downloading', 'verifying', 'loading', 'backfilling', 'updating'];

  const key = $derived(LABELS[state.phase]);
  const busy = $derived(BUSY.includes(state.phase));
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
          <span class="min-w-0 text-fg-3">{t('settings.memory.vector.status.readyDetail')}</span>
        {/if}
      </span>
      {#if busy && fraction !== null}
        <span class="tabular text-fg-3">{t('settings.memory.vector.percent', { percent: Math.round(fraction * 100) })}</span>
      {/if}
    </div>

    {#if busy}
      <ProgressBar label={t('settings.memory.vector.progress.label')} {fraction} />
    {:else if failed && state.error}
      <p class="text-sm text-fg-3">{state.error}</p>
    {/if}
  </div>
{/if}

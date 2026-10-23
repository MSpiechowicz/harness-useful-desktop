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
</script>

{#if key}
  <div class="flex flex-col gap-2 pb-4" role="status">
    <div class="flex items-baseline justify-between gap-3 text-sm">
      <span class={cx(failed ? 'text-danger' : busy ? 'text-fg-2' : 'text-fg-3')}>{t(key)}</span>
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

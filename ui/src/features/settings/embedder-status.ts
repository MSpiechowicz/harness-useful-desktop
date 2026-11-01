import type { MessageKey } from '$lib/i18n/i18n.svelte';
import type { EmbedderPhase } from '$lib/ipc/bindings/EmbedderPhase';

/** The words of an embedder status line. `off` has none: the switch says it. */
export interface EmbedderStatusCopy {
  /** What the embedder says it does, per phase. */
  phases: Partial<Record<EmbedderPhase, MessageKey>>;
  readyDetail: MessageKey;
  /** Takes `{ percent }`. */
  percent: MessageKey;
  progressLabel: MessageKey;
}

export const MEMORY_EMBEDDER_COPY: EmbedderStatusCopy = {
  phases: {
    starting: 'settings.memory.vector.status.starting',
    downloading: 'settings.memory.vector.status.downloading',
    verifying: 'settings.memory.vector.status.verifying',
    loading: 'settings.memory.vector.status.loading',
    backfilling: 'settings.memory.vector.status.backfilling',
    updating: 'settings.memory.vector.status.updating',
    ready: 'settings.memory.vector.status.ready',
    elsewhere: 'settings.memory.vector.status.elsewhere',
    failed: 'settings.memory.vector.status.failed',
  },
  readyDetail: 'settings.memory.vector.status.readyDetail',
  percent: 'settings.memory.vector.percent',
  progressLabel: 'settings.memory.vector.progress.label',
};

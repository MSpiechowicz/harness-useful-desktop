import type { MessageKey } from '$lib/i18n/i18n.svelte';
import type { EmbedderPhase } from '$lib/ipc/bindings/EmbedderPhase';

/** The words of the rows that switch an embedder, show its state and remove its model. `off` has no status line: the switch says it. */
export interface EmbedderStatusCopy {
  /** The vector search switch. */
  title: MessageKey;
  description: MessageKey;
  /** The row that deletes the downloaded model. */
  remove: { title: MessageKey; description: MessageKey; action: MessageKey; confirmTitle: MessageKey; confirmBody: MessageKey };
  /** What the embedder says it does, per phase. */
  phases: Partial<Record<EmbedderPhase, MessageKey>>;
  readyDetail: MessageKey;
  /** Takes `{ percent }`. */
  percent: MessageKey;
  progressLabel: MessageKey;
}

export const MEMORY_EMBEDDER_COPY: EmbedderStatusCopy = {
  title: 'settings.memory.vector.title',
  description: 'settings.memory.vector.description',
  remove: {
    title: 'settings.memory.vector.remove.title',
    description: 'settings.memory.vector.remove.description',
    action: 'settings.memory.vector.remove.action',
    confirmTitle: 'settings.memory.vector.remove.confirmTitle',
    confirmBody: 'settings.memory.vector.remove.confirmBody',
  },
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

export const KNOWLEDGE_EMBEDDER_COPY: EmbedderStatusCopy = {
  title: 'knowledge.settings.vector.title',
  description: 'knowledge.settings.vector.description',
  remove: {
    title: 'knowledge.settings.vector.remove.title',
    description: 'knowledge.settings.vector.remove.description',
    action: 'knowledge.settings.vector.remove.action',
    confirmTitle: 'knowledge.settings.vector.remove.confirmTitle',
    confirmBody: 'knowledge.settings.vector.remove.confirmBody',
  },
  phases: {
    starting: 'knowledge.settings.vector.status.starting',
    downloading: 'knowledge.settings.vector.status.downloading',
    verifying: 'knowledge.settings.vector.status.verifying',
    loading: 'knowledge.settings.vector.status.loading',
    backfilling: 'knowledge.settings.vector.status.backfilling',
    updating: 'knowledge.settings.vector.status.updating',
    ready: 'knowledge.settings.vector.status.ready',
    elsewhere: 'knowledge.settings.vector.status.elsewhere',
    failed: 'knowledge.settings.vector.status.failed',
  },
  readyDetail: 'knowledge.settings.vector.status.readyDetail',
  percent: 'knowledge.settings.vector.percent',
  progressLabel: 'knowledge.settings.vector.progress.label',
};

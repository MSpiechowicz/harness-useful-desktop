import { type FactRow, momentFact } from '$lib/components/facts';
import { t } from '$lib/i18n/i18n.svelte';
import type { ScriptSummary } from '$lib/ipc/bindings/ScriptSummary';
import { formatDateTime, relativeTime } from '$lib/utils/format';

import { APPROVAL_LABELS, authorFacts, exitOutcome, languageLabel, needsReview, PLACE_LABELS, scopeText, shortSha } from './script-format';

function lastExit(script: ScriptSummary): FactRow {
  const label = t('memory.scripts.detail.lastExit');
  const outcome = exitOutcome(script.lastExitCode);

  if (outcome === 'ok') {
    return { label, value: t('memory.scripts.detail.exitOk'), on: true };
  }

  if (outcome === 'failed') {
    return { label, value: t('memory.scripts.detail.exitFailed', { code: script.lastExitCode ?? 0 }), tone: 'danger' };
  }

  return { label, value: '—' };
}

/**
 * The Details card of a script: the same facts in the same order for every script, and what does not apply reads as a dash.
 * `sha256` is the hash of the body that was read, which wins over the list's once it is loaded.
 */
export function scriptFacts(script: ScriptSummary, sha256: string): FactRow[] {
  const lastRun = script.lastRunAt ? { value: relativeTime(script.lastRunAt), title: formatDateTime(script.lastRunAt) } : { value: t('memory.scripts.detail.neverRun') };

  return [
    { label: t('memory.scripts.detail.language'), value: languageLabel(script.language) },
    { label: t('memory.detail.scope'), value: scopeText(script), title: script.repo ?? undefined },
    { label: t('memory.detail.place'), value: t(PLACE_LABELS[script.place]) },
    ...authorFacts(script),
    { label: t('memory.scripts.detail.runs'), value: String(script.runCount) },
    { label: t('memory.scripts.detail.lastRun'), ...lastRun },
    lastExit(script),
    { label: t('memory.scripts.detail.version'), value: shortSha(sha256), title: sha256, mono: true },
    { label: t('memory.scripts.detail.timeout'), value: t('memory.scripts.detail.seconds', { count: script.timeoutSecs }) },
    { label: t('memory.scripts.detail.trust'), value: t(APPROVAL_LABELS[script.approval]), ...(needsReview(script) ? { tone: 'warning' as const } : { on: true }) },
    momentFact(t('memory.detail.updated'), script.updatedAt),
  ];
}

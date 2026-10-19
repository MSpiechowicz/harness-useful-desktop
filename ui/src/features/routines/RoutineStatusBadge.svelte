<script lang="ts">
  import DocStatusBadge from '$lib/components/docs/DocStatusBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RoutineStatus } from '$lib/ipc/bindings/RoutineStatus';

  import { STATUS_LABELS, STATUS_TONES } from './routine-format';

  interface Props {
    status: RoutineStatus;
    /** A run of the routine asks the user something: it is "Needs you" instead of "Running". */
    needsYou?: boolean;
    /** A run waits for a free place: the routine is "Waiting", whatever else it is. */
    queued?: boolean;
    /** Why a paused routine is paused (its folder is missing): shown as the badge's tooltip. */
    problem?: string | null;
  }

  let { status, needsYou = false, queued = false, problem = null }: Props = $props();

  function hintOf(of: RoutineStatus, why: string | null): string | undefined {
    if (of === 'duplicate') {
      return t('routines.status.duplicate.hint');
    }

    return of === 'paused' && why ? why : undefined;
  }
</script>

{#if needsYou && status === 'running'}
  <DocStatusBadge label={t('routines.status.needsYou')} tone="warning" dot />
{:else if queued && status !== 'invalid' && status !== 'pending' && status !== 'duplicate'}
  <DocStatusBadge label={t('routines.status.queued')} tone="info" hint={t('routines.status.queued.hint')} />
{:else}
  <!-- An overridden team routine says why it does not run: the user's own routine with the same id does. -->
  <DocStatusBadge label={t(STATUS_LABELS[status])} tone={STATUS_TONES[status]} dot={status === 'active' || status === 'running'} hint={hintOf(status, problem)} />
{/if}

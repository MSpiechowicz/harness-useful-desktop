<script lang="ts">
  import DocStatusBadge from '$lib/components/docs/DocStatusBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';

  import { RUN_STATUS_LABELS, RUN_STATUS_TONES } from './routine-format';

  interface Props {
    run: Pick<RoutineRun, 'status' | 'needsYou'>;
  }

  let { run }: Props = $props();
</script>

{#if run.needsYou && run.status === 'running'}
  <DocStatusBadge label={t('routines.status.needsYou')} tone="warning" dot />
{:else}
  <DocStatusBadge label={t(RUN_STATUS_LABELS[run.status])} tone={RUN_STATUS_TONES[run.status]} dot={run.status === 'running'} />
{/if}

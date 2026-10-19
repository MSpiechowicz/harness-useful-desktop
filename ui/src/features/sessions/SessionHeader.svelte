<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import PaneHeader from '$lib/components/PaneHeader.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { harness } from '$lib/stores/harness.svelte';
  import { relativeTime } from '$lib/utils/format';

  import RoutineChip from '../routines/RoutineChip.svelte';
  import { sessionTitle } from './session-format';

  interface Props {
    summary: SessionSummary;
  }

  let { summary }: Props = $props();
</script>

{#snippet line()}
  <!-- Only what the details beside the transcript do not say: which harness, and when. -->
  <span class="tabular">{harness.get(summary.ref.harness)?.info.name ?? summary.ref.harness}</span>
  <span class="tabular ml-2">· {t('sessions.header.updated', { time: relativeTime(summary.updatedAt) })}</span>
  {#if summary.routine}<RoutineChip routine={summary.routine} class="ml-2 max-w-60 align-middle" />{/if}
  {#if summary.emptyCopy}
    <Badge class="ml-2 align-middle">{t('sessions.header.emptyCopy')}</Badge>
  {:else if summary.parentSession}
    <Badge tone="info" class="ml-2 align-middle">{t('sessions.header.forked')}</Badge>
  {/if}
{/snippet}

<PaneHeader title={sessionTitle(summary)} subtitle={line} />

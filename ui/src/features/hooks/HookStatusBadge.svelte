<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookStatus } from '$lib/ipc/bindings/HookStatus';

  import { STATUS_LABELS, STATUS_TONES } from './hook-format';

  interface Props {
    status: HookStatus;
  }

  let { status }: Props = $props();
</script>

{#if status === 'duplicate'}
  <!-- An overridden team hook says why it does not run: the user's own hook with the same id does. -->
  <Tooltip text={t('hooks.status.duplicate.hint')}>
    {#snippet children({ describedby })}
      <span aria-describedby={describedby} class="inline-flex"><Badge tone={STATUS_TONES[status]}>{t(STATUS_LABELS[status])}</Badge></span>
    {/snippet}
  </Tooltip>
{:else}
  <Badge tone={STATUS_TONES[status]} dot={status === 'active'}>{t(STATUS_LABELS[status])}</Badge>
{/if}

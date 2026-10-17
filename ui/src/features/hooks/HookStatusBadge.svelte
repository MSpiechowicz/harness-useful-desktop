<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookStatus } from '$lib/ipc/bindings/HookStatus';

  import { STATUS_LABELS, STATUS_TONES } from './hook-format';

  interface Props {
    status: HookStatus;
  }

  let { status }: Props = $props();
</script>

<Badge tone={STATUS_TONES[status]} dot={status === 'active'}>{t(STATUS_LABELS[status])}</Badge>

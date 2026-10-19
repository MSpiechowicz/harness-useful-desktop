<script lang="ts">
  import DocStatusBadge from '$lib/components/docs/DocStatusBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookStatus } from '$lib/ipc/bindings/HookStatus';

  import { STATUS_LABELS, STATUS_TONES } from './hook-format';

  interface Props {
    status: HookStatus;
  }

  let { status }: Props = $props();
</script>

<!-- An overridden team hook says why it does not run: the user's own hook with the same id does. -->
<DocStatusBadge label={t(STATUS_LABELS[status])} tone={STATUS_TONES[status]} dot={status === 'active'} hint={status === 'duplicate' ? t('hooks.status.duplicate.hint') : undefined} />

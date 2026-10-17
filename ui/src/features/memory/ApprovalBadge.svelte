<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ScriptSummary } from '$lib/ipc/bindings/ScriptSummary';

  import { APPROVAL_LABELS } from './script-format';

  interface Props {
    approval: ScriptSummary['approval'];
  }

  let { approval }: Props = $props();

  const TONES = { local: 'neutral', approved: 'success', team: 'info', needed: 'warning' } as const satisfies Record<ScriptSummary['approval'], 'neutral' | 'success' | 'info' | 'warning'>;
</script>

<Badge tone={TONES[approval]}>{t(APPROVAL_LABELS[approval])}</Badge>

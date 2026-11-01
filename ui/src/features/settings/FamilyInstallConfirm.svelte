<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';

  import SetupConfirm from '../onboarding/SetupConfirm.svelte';
  import type { FamilyInstallFlow } from './family-install.svelte';

  interface Props {
    flow: FamilyInstallFlow;
  }

  let { flow }: Props = $props();

  const update = $derived(flow.update());
</script>

{#if flow.plan}
  <SetupConfirm
    bind:open={flow.confirming}
    plan={flow.plan}
    onconfirm={() => void flow.run()}
    title={t(update ? flow.copy.confirm.update : flow.copy.confirm.install)}
    body={t(flow.copy.confirm.body)}
    confirmLabel={t(update ? flow.copy.update : flow.copy.install)}
  />
{/if}

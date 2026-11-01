<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';

  import SetupConfirm from '../onboarding/SetupConfirm.svelte';
  import type { KnowledgeInstallFlow } from './knowledge-install.svelte';

  interface Props {
    flow: KnowledgeInstallFlow;
  }

  let { flow }: Props = $props();

  const update = $derived(flow.update());
</script>

{#if flow.plan}
  <SetupConfirm
    bind:open={flow.confirming}
    plan={flow.plan}
    onconfirm={() => void flow.run()}
    title={update ? t('knowledge.settings.install.confirm.update') : t('knowledge.settings.install.confirm.install')}
    body={t('knowledge.settings.install.confirm.body')}
    confirmLabel={update ? t('knowledge.settings.install.update') : t('knowledge.settings.install.install')}
  />
{/if}

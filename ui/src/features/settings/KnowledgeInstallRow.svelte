<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  import KnowledgeInstallConfirm from '../knowledge/KnowledgeInstallConfirm.svelte';
  import { KnowledgeInstallFlow } from '../knowledge/knowledge-install.svelte';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    /** Harness Knowledge is installed but too old for what the app needs. Otherwise it is not installed at all. */
    update: boolean;
    /** Looks at the installed Harness Knowledge again once the installer ran. */
    onfinished: () => Promise<void>;
  }

  let { update, onfinished }: Props = $props();

  const flow = new KnowledgeInstallFlow(
    () => update,
    () => onfinished(),
  );

  const Icon = $derived(update ? RefreshCw : Download);
  const label = $derived(update ? t('knowledge.settings.install.update') : t('knowledge.settings.install.install'));
</script>

<SettingRow title={update ? t('knowledge.settings.install.outdated.title') : t('knowledge.settings.install.missing.title')} description={update ? t('knowledge.settings.install.outdated.description') : t('knowledge.settings.install.missing.description')}>
  {#snippet control()}
    <div class="flex items-center gap-2">
      <Button variant="ghost" onclick={() => nav.go('ecosystem')}>{t('knowledge.settings.install.ecosystem')}</Button>
      <Button variant="primary" loading={flow.busy} onclick={() => void flow.review()}>
        {#snippet leading()}
          <Icon size={16} aria-hidden="true" />
        {/snippet}
        {flow.phase === 'running' ? t(update ? 'knowledge.settings.install.updating' : 'knowledge.settings.install.installing') : label}
      </Button>
    </div>
  {/snippet}
  {#snippet below()}
    {#if flow.phase === 'failed'}
      <p class="pt-2 text-sm text-danger" role="alert">{flow.failure}</p>
    {/if}
  {/snippet}
</SettingRow>

<KnowledgeInstallConfirm {flow} />

<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  import KnowledgeInstallConfirm from './KnowledgeInstallConfirm.svelte';
  import { KnowledgeInstallFlow } from './knowledge-install.svelte';

  interface Props {
    /** Harness Knowledge is installed but too old. Otherwise it is not installed at all. */
    update: boolean;
    /** Looks at the installed Harness Knowledge again after an install. */
    onfinished: () => Promise<void>;
  }

  let { update, onfinished }: Props = $props();

  const flow = new KnowledgeInstallFlow(
    () => update,
    () => onfinished(),
  );

  const label = $derived(update ? t('knowledge.settings.install.update') : t('knowledge.settings.install.install'));
</script>

<!-- The install, in the middle of a view that cannot work without it: the button, the Ecosystem page as the quiet way, and what went wrong. -->
<div class="flex flex-col items-center gap-3">
  <div class="flex items-center justify-center gap-2">
    <Button variant="primary" loading={flow.busy} onclick={() => void flow.review()}>
      {#snippet leading()}
        {#if update}<RefreshCw size={16} aria-hidden="true" />{:else}<Download size={16} aria-hidden="true" />{/if}
      {/snippet}
      {flow.phase === 'running' ? t(update ? 'knowledge.settings.install.updating' : 'knowledge.settings.install.installing') : label}
    </Button>

    <Button variant="ghost" onclick={() => nav.go('ecosystem')}>{t('knowledge.settings.install.ecosystem')}</Button>
  </div>

  {#if flow.phase === 'running'}
    <p in:rise={{ y: 6, duration: 240 }} class="flex items-center gap-2 text-sm text-fg-2" role="status"><Spinner size={16} />{t(update ? 'knowledge.settings.install.updating' : 'knowledge.settings.install.installing')}</p>
  {:else if flow.phase === 'failed'}
    <p in:rise={{ y: 6, duration: 240 }} class="max-w-md text-sm text-danger" role="alert">{flow.failure}</p>
  {/if}
</div>

<KnowledgeInstallConfirm {flow} />

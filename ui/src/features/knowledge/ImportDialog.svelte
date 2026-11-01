<script lang="ts">
  import Play from '@lucide/svelte/icons/play';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
  import { knowledgeImport as flow } from '$lib/stores/knowledgeImport.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';

  import ImportDrop from './ImportDrop.svelte';
  import ImportProgress from './ImportProgress.svelte';
  import ImportStaged from './ImportStaged.svelte';
  import { knowledgeScopeOptions } from './knowledge-repos';
  import LinksEditor from './LinksEditor.svelte';

  const refused = NOTICE_LOOKS.error;

  const running = $derived(flow.phase === 'running');
  const finished = $derived(flow.phase === 'done');
  const options = $derived(knowledgeScopeOptions(projectScopes.data ?? [], knowledgeDocs.repos.data, knowledgeChanges.pulse?.documents));

  const title = $derived.by(() => {
    if (running) {
      return t('knowledge.import.title.running');
    }

    if (!finished) {
      return t('knowledge.import.title');
    }

    if (flow.run.failure !== null) {
      return t('knowledge.import.title.failed');
    }

    return t(flow.run.cancelled ? 'knowledge.import.title.cancelled' : 'knowledge.import.title.done');
  });

  const description = $derived.by(() => {
    if (running) {
      return t('knowledge.import.description.running');
    }

    if (!finished) {
      return t('knowledge.import.description');
    }

    const counts = flow.run.counts;

    return counts === null ? t('knowledge.import.description.noCounts') : t('knowledge.import.toast.counts', { written: counts.added + counts.updated, failed: counts.failed });
  });
</script>

<!-- Closing the window does not stop an import: the header shows how far it is, and a toast says how it ended. -->
<Dialog bind:open={flow.open} size="lg" {title} {description} onclose={() => flow.close()}>
  {#if flow.phase === 'idle'}
    <ImportDrop />
  {:else if flow.phase === 'staged' && flow.intake}
    <div class="flex flex-col gap-5">
      <ImportStaged intake={flow.intake} />

      <SectionCard flat heading={t('knowledge.import.links.heading')}>
        <p class="mb-3 text-sm text-fg-3">{t('knowledge.import.links.help')}</p>
        <LinksEditor links={flow.links} onchange={(next) => (flow.links = next)} {options} />
      </SectionCard>
    </div>
  {:else}
    <div class="flex flex-col gap-4">
      {#if flow.run.failure !== null}
        <div class="flex items-start gap-3 rounded-md p-3 {refused.box}" role="alert">
          <span class="flex h-5 shrink-0 items-center"><refused.icon class={refused.color} size={ICON.sm} aria-hidden="true" /></span>
          <span class="min-w-0 text-sm text-fg [overflow-wrap:anywhere]">{flow.run.failure}</span>
        </div>
      {/if}

      <ImportProgress run={flow.run} {running} />

      {#if running}
        <p class="text-sm text-fg-3">{t('knowledge.import.keepsRunning')}</p>
      {:else if flow.embeddingPending > 0}
        <div class="flex flex-wrap items-center gap-3 rounded-md bg-info-wash px-3 py-2 text-sm text-fg" role="status">
          <span class="min-w-0 flex-1">{tn('knowledge.embed.notice', flow.embeddingPending)}</span>
          <Button onclick={() => void flow.embedNow()}>{t('knowledge.embed.action')}</Button>
        </div>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    {#if running}
      <Button variant="dangerQuiet" loading={flow.cancelling} onclick={() => void flow.cancel()}>{flow.cancelling ? t('knowledge.import.cancelling') : t('knowledge.import.cancel')}</Button>
      <Button onclick={() => flow.close()}>{t('common.close')}</Button>
    {:else if flow.phase === 'staged'}
      <Button disabled={flow.picking} onclick={() => void flow.pick('files')}>{t('knowledge.import.chooseOther')}</Button>
      <Button variant="primary" onclick={() => void flow.start()}>
        {#snippet leading()}<Play size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('knowledge.import.start')}
      </Button>
    {:else if finished}
      <Button variant="primary" onclick={() => flow.close()}>{t('common.done')}</Button>
    {:else}
      <Button onclick={() => flow.close()}>{t('common.close')}</Button>
    {/if}
  {/snippet}
</Dialog>

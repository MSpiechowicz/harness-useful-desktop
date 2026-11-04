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
  import { countsText, knowledgeImport as flow } from '$lib/stores/knowledgeImport.svelte';
  import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
  import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { uid } from '$lib/utils/ids';
  import { isFolderKey } from '$lib/utils/knowledge-local-path';
  import { goesToTeam } from '$lib/utils/knowledge-match';
  import { choosesPlace, effectivePlace, usesBothPlaces } from '$lib/utils/knowledge-routing';

  import ImportDrop from './ImportDrop.svelte';
  import ImportPlace from './ImportPlace.svelte';
  import ImportProgress from './ImportProgress.svelte';
  import ImportStaged from './ImportStaged.svelte';
  import { knowledgeScopeOptions } from './knowledge-repos';
  import { summaryText } from './knowledge-match-text';
  import LinksEditor from './LinksEditor.svelte';

  const refused = NOTICE_LOOKS.error;

  void knowledgeRemote.load();

  const remote = $derived(knowledgeRemote.value);
  /** Where a document goes is chosen only while a kind of document reads Remote + this computer. */
  const choosing = $derived(remote !== null && remote.routingSupported && choosesPlace(remote.enabled, remote));

  // The team database only takes documents linked to git projects. Whether a link to a project would go there depends on where the import goes, which the user may still change,
  // and per file: one set to update a document is filed where that document is kept, the others where the import files new documents.
  const toTeam = $derived(remote !== null && remote.enabled && goesToTeam(flow.rows, flow.choices, flow.landing, effectivePlace(remote, false, choosing ? flow.place : null)));
  const blocked = $derived(toTeam && flow.links.some((link) => isFolderKey(link.repo)));
  const blockedId = uid('knowledge-import-blocked');

  function start(): void {
    flow.place = choosing && remote ? effectivePlace(remote, flow.links.length === 0, flow.place) : null;
    void flow.start();
  }

  // The match table needs the room: a Harness Knowledge that compares files gets the wider window in every step, so it does not jump between them.
  const wide = $derived(knowledgeStatus.value?.features.docsSimilar === true);
  const summary = $derived(flow.rows === null ? '' : summaryText(flow.rows, flow.choices, flow.landing));

  // A match in another place than the import goes to is added as new unless the user sets it to update: the store needs to know where the import goes.
  $effect(() => {
    if (remote === null) {
      flow.landing = 'unknown';
    } else if (!remote.routingSupported || !usesBothPlaces(remote.enabled, remote)) {
      flow.landing = 'one';
    } else {
      flow.landing = effectivePlace(remote, flow.links.length === 0, flow.place);
    }
  });

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

    return counts === null ? t('knowledge.import.description.noCounts') : countsText(counts);
  });
</script>

<!-- Closing the window does not stop an import: the header shows how far it is, and a toast says how it ended. -->
<Dialog bind:open={flow.open} size={wide ? 'wide' : 'lg'} {title} {description} onclose={() => flow.close()}>
  {#if flow.phase === 'idle'}
    <ImportDrop />
  {:else if flow.phase === 'staged' && flow.intake}
    <div class="flex flex-col gap-5">
      <ImportStaged intake={flow.intake} />

      <SectionCard flat heading={t('knowledge.import.links.heading')} hint={t('knowledge.import.links.help')}>
        <LinksEditor links={flow.links} onchange={(next) => (flow.links = next)} {options} team={toTeam} />
      </SectionCard>

      {#if choosing && remote}
        <ImportPlace {remote} global={flow.links.length === 0} place={flow.place} onchange={(next) => (flow.place = next)} />
      {/if}
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
        <!-- Grey text with its button under it, as every hint of the app: no coloured box (see DocMcpHint). -->
        <div class="flex flex-col items-start gap-3" role="status">
          <p class="text-sm text-fg-3">{tn('knowledge.embed.notice', flow.embeddingPending)}</p>
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
      {#if blocked}
        <span id={blockedId} class="mr-auto self-center text-sm text-fg-3" aria-live="polite">{t('knowledge.import.blocked')}</span>
      {:else if summary}
        <span class="mr-auto self-center text-sm text-fg-3" aria-live="polite">{summary}</span>
      {/if}
      <Button disabled={flow.picking} onclick={() => void flow.pick('files')}>{t('knowledge.import.chooseOther')}</Button>
      <Button variant="primary" disabled={blocked} aria-describedby={blocked ? blockedId : undefined} onclick={start}>
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

<script lang="ts">
  import FileUp from '@lucide/svelte/icons/file-up';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import { refusalText, writtenCount } from './notes-import';
  import { notesImport as flow } from './notes-import-state.svelte';
  import NotesImportResult from './NotesImportResult.svelte';
  import NotesImportReview from './NotesImportReview.svelte';
  import NotesImportStart from './NotesImportStart.svelte';
  import { defaultTarget, projectOfTarget } from './scope-target';

  const refused = NOTICE_LOOKS.error;

  const applying = $derived(flow.stage === 'applying');
  const chosen = $derived(flow.picked.length);
  // The file is read for the project the Scope filter looks at (Global for none), else the open one: the review keeps the notes there.
  const path = $derived(projectOfTarget(defaultTarget(viewState.memoryReach, projectScopes.data ?? [], project.active?.path ?? null)));

  const title = $derived.by(() => {
    switch (flow.stage) {
      case 'applying':
        return t('memory.import.title.applying');
      case 'result':
        return t(flow.failure ? 'memory.import.title.failed' : 'memory.import.title.done');
      default:
        return t('memory.import.title');
    }
  });
  const description = $derived.by(() => {
    switch (flow.stage) {
      case 'start':
        return t('memory.import.description.start');
      case 'review':
        return t('memory.import.description.review');
      case 'refused':
        return t('memory.import.description.refused', { file: flow.refusal?.fileName ?? '' });
      case 'applying':
        return t('memory.import.description.applying');
      case 'result':
        return flow.result ? tn('memory.import.description.done', writtenCount(flow.result)) : t('memory.import.description.failed');
    }
  });
</script>

<!-- While the import runs the dialog cannot be left: no close button, no Escape, no click outside. -->
<Dialog bind:open={flow.open} size="xl" {title} {description} dismissible={!applying}>
  {#if flow.stage === 'start'}
    <NotesImportStart />
  {:else if flow.stage === 'review'}
    <NotesImportReview />
  {:else if flow.stage === 'refused' && flow.refusal}
    <div class="flex flex-col gap-4">
      <div class="flex items-start gap-3 rounded-md p-3 {refused.box}" role="alert">
        <span class="flex h-5 shrink-0 items-center"><refused.icon class={refused.color} size={ICON.sm} aria-hidden="true" /></span>
        <span class="min-w-0 text-sm text-fg [overflow-wrap:anywhere]">{refusalText(flow.refusal)}</span>
      </div>

      <p class="text-sm text-fg-3">{t('memory.import.hint')}</p>
    </div>
  {:else}
    <NotesImportResult />
  {/if}

  {#snippet footer()}
    {#if flow.stage === 'start'}
      <Button variant="primary" loading={flow.picking} onclick={() => void flow.choose(path)}>
        {#snippet leading()}<FileUp size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('memory.import.choose')}
      </Button>
    {:else if flow.stage === 'review'}
      <Button disabled={flow.picking} onclick={() => void flow.choose(path)}>
        {#snippet leading()}<FileUp size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('memory.import.chooseAnother')}
      </Button>
      <Button variant="primary" disabled={chosen === 0} onclick={() => void flow.apply()}>{tn('memory.import.confirm', chosen)}</Button>
    {:else if flow.stage === 'refused'}
      <Button disabled={flow.picking} onclick={() => flow.begin()}>{t('memory.import.showFormat')}</Button>
      <Button variant="primary" loading={flow.picking} onclick={() => void flow.choose(path)}>
        {#snippet leading()}<FileUp size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('memory.import.chooseAnother')}
      </Button>
    {:else if applying}
      <Button variant="primary" disabled loading>{t('memory.import.title.applying')}</Button>
    {:else}
      {#if flow.failure}
        <Button disabled={flow.picking} onclick={() => void flow.choose(path)}>{t('memory.import.chooseAnother')}</Button>
      {/if}
      <Button variant="primary" onclick={() => (flow.open = false)}>{t('common.done')}</Button>
    {/if}
  {/snippet}
</Dialog>

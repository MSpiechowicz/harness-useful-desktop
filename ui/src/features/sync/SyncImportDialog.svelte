<script lang="ts">
  import { untrack } from 'svelte';

  import FileUp from '@lucide/svelte/icons/file-up';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { uid } from '$lib/utils/ids';

  import SyncResult from './SyncResult.svelte';
  import SyncReviewList from './SyncReviewList.svelte';
  import { ImportController } from './sync-import-state.svelte';
  import { onlyManualEdits } from './sync-model';

  interface Props {
    open: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  const flow = new ImportController(ipc);
  const pasteId = uid('sync-paste');
  const errorId = uid('sync-paste-error');

  // Every opening starts from an empty input.
  $effect(() => {
    if (open) {
      untrack(() => flow.reset());
    }
  });

  const applying = $derived(flow.stage === 'applying');
  const chosen = $derived(flow.selectedIds.length);

  const title = $derived(
    {
      input: t('sync.importDialog.title'),
      review: t('sync.importDialog.titleReview'),
      applying: t('sync.importDialog.titleApplying'),
      result: t('sync.importDialog.titleDone'),
    }[flow.stage],
  );
  const description = $derived.by(() => {
    switch (flow.stage) {
      case 'input':
        return t('sync.importDialog.description');
      case 'review':
        return t('sync.importDialog.descriptionReview');
      case 'applying':
        return t('sync.importDialog.descriptionApplying');
      case 'result':
        return flow.failure ? t('sync.result.failed', { reason: flow.failure }) : flow.result?.ok ? t('sync.result.ok') : flow.result && onlyManualEdits(flow.result) ? t('sync.result.manual') : t('sync.result.partial');
    }
  });
</script>

<!-- While the import runs the dialog cannot be left: no close button, no Escape, no click outside. -->
<Dialog bind:open size="xl" {title} {description} dismissible={!applying}>
  {#if flow.stage === 'input'}
    <div class="flex flex-col gap-3">
      <label for={pasteId} class="text-sm font-medium text-fg">{t('sync.importDialog.paste')}</label>

      <textarea
        id={pasteId}
        bind:value={flow.text}
        rows="8"
        spellcheck="false"
        autocomplete="off"
        placeholder={t('sync.importDialog.placeholder')}
        aria-invalid={flow.error ? 'true' : undefined}
        aria-describedby={flow.error ? errorId : undefined}
        class="w-full resize-none rounded-md border bg-surface-2 px-3 py-2 font-mono text-sm break-all text-fg outline-none placeholder:text-fg-3 focus:border-focus {flow.error ? 'border-danger-edge' : 'border-line-strong'}"
      ></textarea>

      {#if flow.error}
        <p id={errorId} class="text-sm text-danger" role="alert">{flow.error}</p>
      {:else if flow.noFile}
        <p class="text-sm text-fg-2" role="status">{t('sync.importDialog.fileCancelled')}</p>
      {/if}

      <p class="text-sm text-fg-3">{t('sync.importDialog.trust')}</p>
    </div>
  {:else if flow.stage === 'review'}
    <SyncReviewList {flow} />
  {:else}
    <SyncResult {flow} />
  {/if}

  {#snippet footer()}
    {#if flow.stage === 'input'}
      <Button disabled={flow.reviewing} onclick={() => void flow.openFile()}>
        {#snippet leading()}<FileUp size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('sync.importDialog.openFile')}
      </Button>
      <Button variant="primary" disabled={flow.text.trim() === ''} loading={flow.reviewing} onclick={() => void flow.review()}>{t('sync.importDialog.review')}</Button>
    {:else if flow.stage === 'review'}
      <Button onclick={() => flow.back()}>{t('common.back')}</Button>
      <Button variant="primary" disabled={chosen === 0} onclick={() => void flow.apply()}>{t('sync.review.confirm', { count: chosen })}</Button>
    {:else if applying}
      <Button variant="primary" disabled loading>{t('sync.importDialog.titleApplying')}</Button>
    {:else}
      <Button variant="primary" onclick={() => (open = false)}>{t('common.done')}</Button>
    {/if}
  {/snippet}
</Dialog>

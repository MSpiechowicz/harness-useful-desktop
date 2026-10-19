<script lang="ts">
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { ICON } from '$lib/components/icon';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';

  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import FailureBox from './FailureBox.svelte';
  import { discardLocal } from './notes-discard';
  import { copiedCount, copyDescription, copyTitle, failedNotes, NotesCopyFlow } from './notes-copy-state.svelte';
  import { COPY_TEXT } from './notes-copy-text';
  import NotesCopyStats from './NotesCopyStats.svelte';
  import NotesCopyTable from './NotesCopyTable.svelte';

  interface Props {
    open: boolean;
    /** What is copied: the notes or the scripts saved on this computer. */
    kind?: MemoryCopyKind;
    /** The dialog was closed: Not now, Done, the close button or Escape. */
    onclose: () => void;
  }

  let { open, kind = 'notes', onclose }: Props = $props();

  // A new kind starts a new comparison: the flow belongs to one kind.
  const flow = $derived(new NotesCopyFlow(kind));
  const text = $derived(COPY_TEXT[kind]);

  let confirmer: SessionConfirm | undefined = $state();

  const counts = $derived(flow.preview?.counts);
  const result = $derived(flow.result);
  // Only what the copy left on purpose is offered for deletion, by id: not the notes that failed, and not notes saved after the comparison.
  const leftover = $derived(result?.leftover ?? []);

  const reviewStats = $derived([
    { key: 'added', label: t('memory.copy.stat.added'), value: (counts?.added ?? 0) + (counts?.updated ?? 0) },
    { key: 'identical', label: t('memory.copy.stat.identical'), value: counts?.identical ?? 0 },
    { key: 'teamNewer', label: t('memory.copy.stat.teamNewer'), value: counts?.teamNewer ?? 0 },
    { key: 'conflicts', label: t('memory.copy.stat.conflicts'), value: counts?.conflicts ?? 0 },
    { key: 'blocked', label: t('memory.copy.stat.blocked'), value: counts?.failed ?? 0 },
  ]);
  const resultStats = $derived([
    { key: 'copied', label: t('memory.copy.result.copied'), value: result ? copiedCount(result) : 0 },
    { key: 'stays', label: t('memory.copy.result.stays'), value: result?.kept ?? 0 },
    { key: 'failed', label: t('memory.copy.result.failed'), value: result ? failedNotes(result) : 0 },
  ]);

  // Opening the dialog compares at once. Closing it forgets the comparison, so the next opening compares again.
  $effect(() => {
    const current = flow;

    if (open) {
      untrack(() => void current.begin());
    } else {
      untrack(() => current.reset());
    }
  });

  async function deleteLeftover(): Promise<void> {
    if (confirmer && (await discardLocal(kind, { ids: leftover }, (request) => confirmer!.ask(request)))) {
      flow.discarded();
    }
  }
</script>

<!-- While the copy runs the dialog cannot be left: no close button, no Escape, no click outside. -->
<Dialog {open} size="xl" title={copyTitle(flow)} description={copyDescription(flow)} dismissible={flow.stage !== 'applying'} {onclose}>
  {#if flow.stage === 'loading' || flow.stage === 'applying'}
    <div class="flex items-center gap-3 py-2 text-sm text-fg-2" role="status">
      <Spinner size={ICON.md} />
      <span>{t(flow.stage === 'loading' ? 'memory.copy.loading' : 'memory.copy.title.applying')}</span>
    </div>
  {:else if flow.stage === 'failed'}
    <FailureBox message={flow.failure ?? ''} />
  {:else if flow.stage === 'review' && flow.preview && flow.preview.total > 0}
    <div class="@container flex flex-col gap-5">
      <NotesCopyStats stats={reviewStats} />

      <p class="text-sm text-fg-3">{t(text.statHelp)}</p>

      {#if flow.conflicts > 0}
        <section class="flex flex-col gap-3" aria-label={t(text.conflictsTitle)}>
          <div class="flex flex-col gap-0.5">
            <h3 class="text-sm font-medium text-fg">{t(text.conflictsTitle)}</h3>
            <p class="text-sm text-fg-3">{t(text.conflictsHelp)}</p>
          </div>

          <div class="flex items-center justify-between gap-3">
            <p class="tabular text-sm text-fg-2" role="status">{t('memory.copy.selected', { picked: flow.pickedCount, total: flow.conflicts })}</p>

            <div class="flex items-center gap-1">
              <Button variant="ghost" disabled={flow.pickedCount === flow.conflicts} onclick={() => flow.toggleAll(true)}>{t('memory.copy.selectAll')}</Button>
              <Button variant="ghost" disabled={flow.pickedCount === 0} onclick={() => flow.toggleAll(false)}>{t('memory.copy.selectNone')}</Button>
            </div>
          </div>

          <NotesCopyTable
            {kind}
            rows={flow.rows}
            page={flow.page}
            total={flow.conflicts}
            busy={flow.paging}
            picked={(id) => flow.isPicked(id)}
            ontoggle={(id, on) => flow.toggle(id, on)}
            onpage={(next) => void flow.go(next)}
          />
        </section>
      {/if}

      {#if flow.blocked > 0}
        <section class="flex flex-col gap-3" aria-label={tn(text.blockedTitle, flow.blocked)}>
          <div class="flex flex-col gap-0.5">
            <h3 class="text-sm font-medium text-fg">{tn(text.blockedTitle, flow.blocked)}</h3>
            {#if flow.copyable > 0}
              <p class="text-sm text-fg-3">{t(text.blockedHelp)}</p>
            {/if}
          </div>

          <NotesCopyTable {kind} blocked rows={flow.blockedRows} page={flow.blockedPage} total={flow.blocked} busy={flow.blockedPaging} onpage={(next) => void flow.goBlocked(next)} />
        </section>
      {/if}
    </div>
  {:else if flow.stage === 'result'}
    <div class="@container flex flex-col gap-4">
      <NotesCopyStats stats={resultStats} />

      {#if result && result.failed.length > 0}
        <p class="text-sm text-warning" role="status">{t(text.failedHelp)}</p>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    {#if flow.stage === 'review' && flow.preview && flow.preview.total > 0}
      <Button onclick={onclose}>{t('memory.copy.notNow')}</Button>
      {#if flow.copyable > 0}
        <Button variant="primary" disabled={flow.paging} onclick={() => void flow.apply()}>{t('memory.copy.confirm')}</Button>
      {:else}
        <Button variant="primary" onclick={onclose}>{t('common.done')}</Button>
      {/if}
    {:else if flow.stage === 'failed'}
      <Button onclick={onclose}>{t('common.close')}</Button>
      <Button variant="primary" onclick={() => void flow.begin()}>{t('memory.tryAgain')}</Button>
    {:else if flow.stage === 'applying'}
      <Button variant="primary" disabled loading>{t('memory.copy.title.applying')}</Button>
    {:else if flow.stage === 'result'}
      {#if leftover.length > 0}
        <Button variant="dangerQuiet" onclick={() => void deleteLeftover()}>{tn(text.leftover, leftover.length)}</Button>
      {/if}
      <Button variant="primary" onclick={onclose}>{t('common.done')}</Button>
    {:else if flow.stage === 'review'}
      <Button variant="primary" onclick={onclose}>{t('common.done')}</Button>
    {/if}
  {/snippet}
</Dialog>

<SessionConfirm bind:this={confirmer} />

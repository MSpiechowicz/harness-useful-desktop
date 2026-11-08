<script lang="ts">
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { ICON } from '$lib/components/icon';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
  import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';

  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import FailureBox from './FailureBox.svelte';
  import { discardLocal } from './notes-discard';
  import { copiedCount, copyDescription, copyTitle, failedNotes } from './notes-copy-headings';
  import { NotesCopyFlow } from './notes-copy-state.svelte';
  import { COPY_TEXT, pickText } from './notes-copy-text';
  import NotesCopyStats from './NotesCopyStats.svelte';
  import NotesCopyTable from './NotesCopyTable.svelte';
  import PickAllActions from './PickAllActions.svelte';

  interface Props {
    open: boolean;
    /** What is copied: the notes or the scripts saved on this computer. */
    kind?: MemoryCopyKind;
    /**
     * The ways to copy that the Sync button offers, the ones turned on in Settings. With two a choice between them shows above the review, the first
     * is the one it opens on. Without it the dialog copies what is stranded on this computer to the remote database, as the banner and Settings ask.
     */
    directions?: readonly NotesCopyMode[];
    /** The dialog was closed: Not now, Done, the close button or Escape. */
    onclose: () => void;
  }

  let { open, kind = 'notes', directions, onclose }: Props = $props();

  /** The direction the user chose in this opening, if they chose one. */
  let chosen = $state<NotesCopyMode | null>(null);

  const mode = $derived<NotesCopyMode>(chosen !== null && directions?.includes(chosen) ? chosen : (directions?.[0] ?? 'stranded'));
  const choices = $derived(directions && directions.length > 1 ? directions : null);

  // A new kind or direction starts a new comparison: the flow belongs to one of each.
  const flow = $derived(new NotesCopyFlow(kind, mode));
  const text = $derived(COPY_TEXT[kind]);
  const pick = $derived(pickText(kind, mode));

  let confirmer: SessionConfirm | undefined = $state();

  const counts = $derived(flow.preview?.counts);
  const result = $derived(flow.result);
  // Only what the copy left on purpose is offered for deletion, by id: not the notes that failed, and not notes saved after the comparison.
  // Only for stranded entries: in a scope that reads both places the server cannot discard what stays, so the other directions offer nothing.
  const leftover = $derived(mode === 'stranded' ? (result?.leftover ?? []) : []);

  const toRemoteStats = $derived([
    { key: 'added', label: t('memory.copy.stat.added'), value: (counts?.added ?? 0) + (counts?.updated ?? 0) },
    { key: 'identical', label: t('memory.copy.stat.identical'), value: counts?.identical ?? 0 },
    { key: 'teamNewer', label: t('memory.copy.stat.teamNewer'), value: counts?.teamNewer ?? 0 },
    { key: 'conflicts', label: t('memory.copy.stat.conflicts'), value: counts?.conflicts ?? 0 },
    { key: 'blocked', label: t('memory.copy.stat.blocked'), value: counts?.failed ?? 0 },
  ]);
  // To this computer the remote stays as it is: what can be picked, what is here already, what differs here, what is blocked.
  const toLocalStats = $derived([
    { key: 'copyable', label: t('memory.sync.stat.copyable'), value: counts?.added ?? 0 },
    { key: 'identical', label: t('memory.sync.stat.identical'), value: counts?.identical ?? 0 },
    { key: 'localDiffers', label: t('memory.sync.stat.localDiffers'), value: counts?.localDiffers ?? 0 },
    { key: 'blocked', label: t('memory.copy.stat.blocked'), value: counts?.failed ?? 0 },
  ]);
  const reviewStats = $derived(flow.pulls ? toLocalStats : toRemoteStats);
  const resultStats = $derived(
    flow.pulls
      ? [
          { key: 'copied', label: t('memory.sync.result.copied'), value: result ? copiedCount(result, mode) : 0 },
          { key: 'failed', label: t('memory.copy.result.failed'), value: result ? failedNotes(result, mode) : 0 },
        ]
      : [
          { key: 'copied', label: t('memory.copy.result.copied'), value: result ? copiedCount(result) : 0 },
          { key: 'stays', label: t('memory.copy.result.stays'), value: result?.kept ?? 0 },
          { key: 'failed', label: t('memory.copy.result.failed'), value: result ? failedNotes(result) : 0 },
        ],
  );

  /** Whether the table has rows to show: the conflicts, or when picking every entry the comparison found. */
  const listed = $derived(flow.picking ? (flow.preview?.total ?? 0) > 0 : flow.conflicts > 0);
  /** When picking the table can be narrowed to the rows that can be picked, when there are others. */
  const narrowable = $derived(flow.picking && flow.selectable > 0 && (flow.preview?.total ?? 0) > flow.selectable);
  const tableTotal = $derived(flow.picking && flow.view === 'pickable' ? flow.selectable : flow.picking ? (flow.preview?.total ?? 0) : flow.conflicts);

  /** Why Tick all and Untick all have nothing to do right now, or null when they can be pressed. */
  const allReason = $derived(flow.pickedCount === flow.selectable ? t('memory.sync.pick.allDone') : null);
  const noneReason = $derived(flow.pickedCount === 0 ? t('memory.sync.pick.noneYet') : null);

  // Opening the dialog compares at once. Closing it forgets the comparison, so the next opening compares again.
  $effect(() => {
    const current = flow;

    if (open) {
      untrack(() => void current.begin());
    } else {
      untrack(() => {
        current.reset();
        chosen = null;
      });
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
  <!-- The choice stays above the comparison, so switching direction never moves it. It is not offered while the copy runs or after it ran. -->
  {#if choices && flow.stage !== 'applying' && flow.stage !== 'result'}
    <div class="mb-5 flex">
      <SegmentedControl
        options={choices.map((value) => ({ value, label: t(`memory.sync.direction.${value as 'toRemote' | 'toLocal'}`) }))}
        value={mode}
        label={t('memory.sync.direction')}
        onchange={(next) => (chosen = next)}
      />
    </div>
  {/if}

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

      {#if !flow.pulls}
        <p class="text-sm text-fg-3">{t(text.statHelp)}</p>
      {/if}

      {#if listed}
        {@const pickTitle = flow.picking ? t(pick.pickTitle) : t(text.conflictsTitle)}
        <section class="flex flex-col gap-3" aria-label={pickTitle}>
          <div class="flex flex-col gap-0.5">
            <h3 class="text-sm font-medium text-fg">{pickTitle}</h3>
            {#if !flow.picking}
              <p class="text-sm text-fg-3">{t(text.conflictsHelp)}</p>
            {/if}
          </div>

          {#if flow.selectable > 0}
            <div class="flex flex-wrap items-center justify-between gap-3">
              <p class="tabular text-sm text-fg-2" role="status">{t(flow.picking ? 'memory.sync.selected' : 'memory.copy.selected', { picked: flow.pickedCount, total: flow.selectable })}</p>

              <div class="flex items-center gap-3">
                {#if narrowable}
                  <SegmentedControl
                    options={[
                      { value: 'pickable', label: t('memory.sync.view.pickable') },
                      { value: 'all', label: t('memory.sync.view.all') },
                    ]}
                    value={flow.view === 'all' ? 'all' : 'pickable'}
                    label={t('memory.sync.view')}
                    onchange={(next) => void flow.show(next)}
                  />
                {/if}

                <PickAllActions
                  selectAllLabel={t('memory.copy.selectAll')}
                  selectNoneLabel={t('memory.copy.selectNone')}
                  {allReason}
                  {noneReason}
                  onall={() => flow.toggleAll(true)}
                  onnone={() => flow.toggleAll(false)}
                />
              </div>
            </div>
          {/if}

          <NotesCopyTable
            {kind}
            {mode}
            rows={flow.rows}
            page={flow.page}
            total={tableTotal}
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
              <p class="text-sm text-fg-3">{t(pick.blockedHelp)}</p>
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
        <p class="text-sm text-warning" role="status">{t(pick.failedHelp)}</p>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    {#if flow.stage === 'review' && flow.preview && flow.preview.total > 0}
      <Button onclick={onclose}>{t('memory.copy.notNow')}</Button>
      {#if flow.copyable > 0}
        <!-- When picking nothing is ticked at first, and copying needs a tick: the button says so while it waits for one. -->
        <Tooltip text={t('memory.sync.pickFirst')} disabled={!flow.picking || flow.pickedCount > 0}>
          {#snippet children({ describedby })}
            <Button variant="primary" aria-describedby={describedby} disabled={flow.paging || (flow.picking && flow.pickedCount === 0)} onclick={() => void flow.apply()}>{t('memory.copy.confirm')}</Button>
          {/snippet}
        </Tooltip>
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

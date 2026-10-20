<script lang="ts">
  import Play from '@lucide/svelte/icons/play';
  import Square from '@lucide/svelte/icons/square';

  import Button from '$lib/components/Button.svelte';
  import DocActions from '$lib/components/docs/DocActions.svelte';
  import type { DocActionHandlers } from '$lib/components/docs/doc-groups';
  import { ICON } from '$lib/components/icon';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';

  import { isQueued } from './routine-format';
  import { COPY_BLOCK_TEXT, type CopyBlocked, RUN_BLOCK_TEXT, SHARE_BLOCK_TEXT } from './routine-team';

  interface Props extends DocActionHandlers {
    view: RoutineView;
    /** Why Edit is off (a write would approve what it writes), or null. */
    lockReason: string | null;
    /** Routines are shared with the team, so a local routine can be shared. */
    sharing: boolean;
    /** The local original of a team routine is there to update the team copy from. */
    canUpdate: boolean;
    /** A folder of the repository of a team routine is there on this computer, which a copy is kept in. */
    projectKnown: boolean;
    /** A run of the routine is going on. */
    running: boolean;
    /** A change to this routine is being written. */
    busy: boolean;
    onrun: () => void;
    onstop: () => void;
  }

  let { view, lockReason, sharing, canUpdate, projectKnown, running, busy, onrun, onstop, onedit, ondelete, onshare, onupdate, onremove, oncopy }: Props = $props();

  const labels = $derived({ edit: t('routines.panel.edit'), share: t('routines.share.action'), delete: t('routines.panel.delete'), copy: t('routines.team.copy'), update: t('routines.team.update'), remove: t('routines.team.remove') });
  const blocked = $derived(view.shareBlock === null ? null : t(SHARE_BLOCK_TEXT[view.shareBlock]));

  // A team routine is copied only once it can run: one that waits for approval, cannot run or is overridden is never offered.
  const copyBlock = $derived.by(() => {
    const key = COPY_BLOCK_TEXT[view.status as CopyBlocked];

    if (key !== undefined) {
      return t(key);
    }

    // A copy of a routine of a repository is kept in a folder of it.
    return view.origin === 'team' && view.teamRepo !== null && !projectKnown ? t('routines.team.copy.noProject') : null;
  });

  // Run now needs a routine that is valid and approved. A pending one never runs, not even by hand. One that waits for a free place starts by itself.
  const runBlock = $derived.by(() => {
    if (running) {
      return null;
    }

    if (isQueued(view)) {
      return t('routines.run.block.queued');
    }

    // A paused routine has no folder to run in, whatever its approval says.
    if (view.canRunNow && view.status !== 'paused') {
      return null;
    }

    return t(RUN_BLOCK_TEXT[view.status] ?? 'routines.run.block.unknown');
  });
</script>

{#snippet runButtons()}
  {#if running}
    <Button variant="dangerQuiet" disabled={busy} onclick={onstop}>
      {#snippet leading()}<Square size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('routines.run.stop')}
    </Button>
  {:else}
    <Tooltip text={runBlock ?? ''} disabled={runBlock === null}>
      {#snippet children({ describedby })}
        <Button variant="primary" disabled={busy || runBlock !== null} aria-describedby={describedby} onclick={onrun}>
          {#snippet leading()}<Play size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('routines.run.now')}
        </Button>
      {/snippet}
    </Tooltip>
  {/if}
{/snippet}

<!-- A team routine is the team's: it is copied, updated from its original or removed, and never edited or deleted here. -->
<DocActions {labels} team={view.origin === 'team'} {lockReason} {sharing} shareBlock={blocked} {copyBlock} canUpdate={view.teamChanged && canUpdate} {busy} leading={runButtons} {onedit} {ondelete} {onshare} {onupdate} {onremove} {oncopy} />

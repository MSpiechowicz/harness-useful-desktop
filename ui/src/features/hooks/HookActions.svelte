<script lang="ts">
  import DocActions from '$lib/components/docs/DocActions.svelte';
  import type { DocActionHandlers } from '$lib/components/docs/doc-groups';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookView } from '$lib/ipc/bindings/HookView';

  import { COPY_BLOCK_TEXT, type CopyBlocked, SHARE_BLOCK_TEXT } from './hook-team';

  interface Props extends DocActionHandlers {
    view: HookView;
    /** Why Edit is off (a write would approve what it writes), or null. */
    lockReason: string | null;
    /** Hooks are shared with the team, so a local hook can be shared. */
    sharing: boolean;
    /** The local original of a team hook is there to update the team copy from. */
    canUpdate: boolean;
    /** A change to this hook is being written. */
    busy: boolean;
  }

  let { view, lockReason, sharing, canUpdate, busy, onedit, ondelete, onshare, onupdate, onremove, oncopy }: Props = $props();

  const labels = $derived({ edit: t('hooks.panel.edit'), share: t('hooks.share.action'), delete: t('hooks.panel.delete'), copy: t('hooks.team.copy'), update: t('hooks.team.update'), remove: t('hooks.team.remove') });
  const blocked = $derived(view.shareBlock === null ? null : t(SHARE_BLOCK_TEXT[view.shareBlock]));

  // A team hook is copied only once it can run: one that waits for approval, cannot run or is overridden is never offered.
  const copyBlock = $derived.by(() => {
    const key = COPY_BLOCK_TEXT[view.status as CopyBlocked];

    return key === undefined ? null : t(key);
  });
</script>

<!-- A team hook is the team's: it is copied, updated from its original or removed, and never edited or deleted here. -->
<DocActions {labels} team={view.origin === 'team'} {lockReason} {sharing} shareBlock={blocked} {copyBlock} canUpdate={view.teamChanged && canUpdate} {busy} {onedit} {ondelete} {onshare} {onupdate} {onremove} {oncopy} />

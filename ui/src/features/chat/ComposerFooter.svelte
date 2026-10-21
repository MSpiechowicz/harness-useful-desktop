<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import GitCompare from '@lucide/svelte/icons/git-compare';

  import CostChip from '$lib/components/CostChip.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ContextMeter from './ContextMeter.svelte';
  import ProjectPicker from './ProjectPicker.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const CHIP =
    'inline-flex h-control min-w-0 items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg';

  // Git facts belong to the open project; a chat that sits in another one shows none.
  const status = $derived(git.status?.isRepo && git.activeProject === session.info.project ? git.status : null);
  const branch = $derived(status ? (status.branch ?? t('chat.footer.detached')) : '');
  const changed = $derived(status?.entries.length ?? 0);
  const costArgs = $derived(session.chat.session?.sessionId ? { session_id: session.chat.session.sessionId } : null);
</script>

<!-- The right padding plus the meters' own 6px puts the last text 16px from the edge, under the send button's right edge.
     Where the chat works (project, branch, changes) on the left, how much it has used on the right. The branch gives
     way first when the card is narrow; below 460px the project shrinks to its icon. -->
<div class="flex items-center gap-1 border-t border-line pr-2.5 pb-1 pl-2 pt-1">
  <ProjectPicker {session} />

  {#if status}
    <span title={branch} class="inline-flex h-control min-w-0 shrink-[100] items-center gap-1.5 px-2 text-sm text-fg-3">
      <span class="sr-only">{t('chat.footer.branchLabel')}</span>
      <GitBranch size={14} aria-hidden="true" class="shrink-0" />
      <span class="min-w-0 -my-[3px] truncate py-[3px] [text-box:trim-both_cap_alphabetic]">{branch}</span>
      {#if status.ahead > 0}
        <span class="tabular inline-flex shrink-0 items-center text-xs text-success"><ArrowUp size={12} aria-hidden="true" />{status.ahead}<span class="sr-only">{t('chat.footer.ahead', { count: status.ahead })}</span></span>
      {/if}
      {#if status.behind > 0}
        <span class="tabular inline-flex shrink-0 items-center text-xs text-warning"><ArrowDown size={12} aria-hidden="true" />{status.behind}<span class="sr-only">{t('chat.footer.behind', { count: status.behind })}</span></span>
      {/if}
    </span>

    {#if changed > 0}
      <button type="button" title={t('chat.footer.openChanges')} onclick={() => nav.go('changes')} class="{CHIP} shrink-0 text-warning hover:bg-warning-wash hover:text-warning">
        <GitCompare size={16} aria-hidden="true" class="shrink-0" />
        <span class="tabular [text-box:trim-both_cap_alphabetic]"><span class="@max-[460px]:sr-only">{t('chat.footer.changed', { count: changed })}</span><span class="@min-[461px]:hidden" aria-hidden="true">{changed}</span></span>
      </button>
    {/if}
  {/if}

  <!-- With no model there is no context to fill and nothing to pay for, so the empty meter and its dash are not shown. -->
  {#if !session.noModels}
    <div class="ml-auto flex shrink-0 items-center">
      <!-- Shown once the harness reports how full the context is: an unknown amount says nothing. -->
      {#if session.chat.session?.contextUsage}
        <ContextMeter usage={session.chat.session.contextUsage} />
      {/if}
      <CostChip kind="session" args={costArgs} refreshKey={session.streaming} align="end" side="top" quiet />
    </div>
  {/if}
</div>

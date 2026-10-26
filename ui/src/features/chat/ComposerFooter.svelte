<script lang="ts">
  import GitCompare from '@lucide/svelte/icons/git-compare';

  import CostChip from '$lib/components/CostChip.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import BranchChip from './branch/BranchChip.svelte';
  import { CHIP } from './branch/chip';
  import ContextMeter from './ContextMeter.svelte';
  import ProjectPicker from './ProjectPicker.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  // Git facts belong to the open project; a chat that sits in another one shows none.
  const status = $derived(git.status?.isRepo && git.activeProject === session.info.project ? git.status : null);
  const changed = $derived(status?.entries.length ?? 0);
  const costArgs = $derived(session.chat.session?.sessionId ? { session_id: session.chat.session.sessionId } : null);
</script>

<!-- The right padding plus the meters' own 6px puts the last text 16px from the edge, under the send button's right edge.
     Where the chat works (project, branch, changes) on the left, how much it has used on the right. The branch gives
     way first when the card is narrow; below 460px the project shrinks to its icon. -->
<div class="flex items-center gap-1 border-t border-line py-(--footer-pad) pr-2.5 pl-2">
  <ProjectPicker {session} />

  {#if status}
    <BranchChip {session} />

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

<script lang="ts">
  import CostChip from '$lib/components/CostChip.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { harness } from '$lib/stores/harness.svelte';

  import ApprovalBadge from './ApprovalBadge.svelte';
  import StatsPopover from './StatsPopover.svelte';
  import ThinkingToggle from './ThinkingToggle.svelte';
  import TrustOnce from './TrustOnce.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const summary = $derived(harness.get(session.info.harness));
  const capabilities = $derived(summary?.capabilities);
  const sessionId = $derived(session.chat.session?.sessionId ?? null);
  const costArgs = $derived(sessionId ? { session_id: sessionId } : null);
</script>

<header class="page-gutter flex items-center gap-3 border-b border-line py-1.5">
  <div class="flex shrink-0 items-center gap-2">
    <CostChip kind="session" args={costArgs} refreshKey={session.streaming} />
  </div>

  <div class="ml-auto flex shrink-0 items-center gap-2">
    {#if capabilities?.approvalModes}<ApprovalBadge {session} />{/if}
    {#if capabilities?.projectTrust}<TrustOnce {session} />{/if}
    <ThinkingToggle />
    <StatsPopover {session} />
  </div>
</header>

<script lang="ts">
  import CostChip from '$lib/components/CostChip.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { harness } from '$lib/stores/harness.svelte';

  import ApprovalBadge from './ApprovalBadge.svelte';
  import ContextMeter from './ContextMeter.svelte';
  import ModelPicker from './ModelPicker.svelte';
  import SessionName from './SessionName.svelte';
  import StatsPopover from './StatsPopover.svelte';
  import ThinkingPicker from './ThinkingPicker.svelte';
  import TrustOnce from './TrustOnce.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const summary = $derived(harness.get(session.info.harness));
  const capabilities = $derived(summary?.capabilities);
  const sessionId = $derived(session.chat.session?.sessionId ?? null);
  const costArgs = $derived(sessionId ? { session_id: sessionId } : null);
  const projectName = $derived(session.info.project.split(/[\\/]/).filter(Boolean).at(-1) ?? session.info.project);
</script>

<header class="page-gutter flex items-center gap-3 border-b border-line py-2">
  {#if summary}<HarnessBadge info={summary.info} size="sm" />{/if}
  <SessionName {session} />

  <div class="flex shrink-0 items-center gap-2">
    <ModelPicker {session} />
    <ThinkingPicker {session} />
    <ContextMeter usage={session.chat.session?.contextUsage ?? null} />
    <CostChip kind="session" args={costArgs} refreshKey={session.streaming} />
  </div>

  <div class="ml-auto flex shrink-0 items-center gap-2">
    <span class="max-w-40 truncate text-sm text-fg-3 max-[1100px]:hidden" title={session.info.project}>{projectName}</span>
    {#if capabilities?.approvalModes}<ApprovalBadge {session} />{/if}
    {#if capabilities?.projectTrust}<TrustOnce {session} />{/if}
    <StatsPopover {session} />
  </div>
</header>

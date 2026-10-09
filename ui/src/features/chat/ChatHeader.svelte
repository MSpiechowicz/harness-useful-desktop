<script lang="ts">
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
  const projectName = $derived(session.info.project.split(/[\\/]/).filter(Boolean).at(-1) ?? session.info.project);
</script>

<header class="flex flex-col gap-2.5 border-b border-line px-6 py-3">
  <div class="flex items-center gap-3">
    {#if summary}<HarnessBadge info={summary.info} size="sm" />{/if}
    <SessionName {session} />
    <span class="ml-auto truncate text-sm text-fg-3" title={session.info.project}>{summary?.info.name ?? session.info.harness} · {projectName}</span>
  </div>

  <div class="flex flex-wrap items-center gap-2">
    <ModelPicker {session} />
    <ThinkingPicker {session} />
    <ContextMeter usage={session.chat.session?.contextUsage ?? null} />

    <div class="ml-auto flex items-center gap-2">
      {#if capabilities?.approvalModes}<ApprovalBadge {session} />{/if}
      {#if capabilities?.projectTrust}<TrustOnce {session} />{/if}
      <StatsPopover {session} />
    </div>
  </div>
</header>

<script lang="ts">
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

  const capabilities = $derived(harness.get(session.info.harness)?.capabilities);
</script>

<!-- The open chat's controls, in the top bar's right side: how it asks before acting, how much it thinks, what it used. -->
<div class="flex shrink-0 items-center gap-2">
  {#if capabilities?.approvalModes}<ApprovalBadge {session} />{/if}
  {#if capabilities?.projectTrust}<TrustOnce {session} />{/if}
  <ThinkingToggle />
  <StatsPopover {session} />
</div>

<script lang="ts">
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ApprovalBadge from './ApprovalBadge.svelte';
  import MemoryButton from './MemoryButton.svelte';
  import TrustOnce from './TrustOnce.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const capabilities = $derived(harness.get(session.info.harness)?.capabilities);
  // A harness that starts only in a folder it trusts (Claude Code) has nothing left to trust in a running chat: it only
  // shows that this chat trusted the project once. One that has nothing of this project to leave out (Codex without an
  // untrusted .codex layer) offers no trust button either.
  const showTrust = $derived(
    Boolean(capabilities?.projectTrust) && (session.info.trustedOnce || (!capabilities?.projectTrustRequired && session.info.trustOffered)),
  );
</script>

<!-- The open chat's controls, in the top bar's right side: how it asks before acting, and Harness Memory. What it shows and what it used are in the "⋯" menu (ChatMenu). -->
<div class="flex shrink-0 items-center gap-2">
  {#if capabilities?.approvalModes}<ApprovalBadge {session} />{/if}
  {#if showTrust}<TrustOnce {session} />{/if}
  <MemoryButton {session} />
</div>

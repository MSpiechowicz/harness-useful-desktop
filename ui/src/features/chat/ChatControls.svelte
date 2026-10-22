<script lang="ts">
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ApprovalBadge from './ApprovalBadge.svelte';
  import MemoryButton from './MemoryButton.svelte';
  import StatsPopover from './StatsPopover.svelte';
  import TrustOnce from './TrustOnce.svelte';
  import ViewToggle from './ViewToggle.svelte';

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

<!-- The open chat's controls, in the top bar's right side: how it asks before acting, what it shows, what it used. -->
<div class="flex shrink-0 items-center gap-2">
  {#if capabilities?.approvalModes}<ApprovalBadge {session} />{/if}
  {#if showTrust}<TrustOnce {session} />{/if}
  <MemoryButton {session} />
  <ViewToggle pref="showThinking" label={t('chat.thinking.toggle')} aria={t('chat.thinking.toggleAria')} hideTitle={t('chat.thinking.hide')} showTitle={t('chat.thinking.show')} />
  <ViewToggle pref="showMessageUsage" label={t('chat.usage.toggle')} aria={t('chat.usage.toggleAria')} hideTitle={t('chat.usage.hide')} showTitle={t('chat.usage.show')} />
  <StatsPopover {session} />
</div>

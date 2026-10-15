<script lang="ts">
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ApprovalBadge from './ApprovalBadge.svelte';
  import StatsPopover from './StatsPopover.svelte';
  import TrustOnce from './TrustOnce.svelte';
  import ViewToggle from './ViewToggle.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const capabilities = $derived(harness.get(session.info.harness)?.capabilities);
</script>

<!-- The open chat's controls, in the top bar's right side: how it asks before acting, what it shows, what it used. -->
<div class="flex shrink-0 items-center gap-2">
  {#if capabilities?.approvalModes}<ApprovalBadge {session} />{/if}
  {#if capabilities?.projectTrust}<TrustOnce {session} />{/if}
  <ViewToggle pref="showThinking" label={t('chat.thinking.toggle')} aria={t('chat.thinking.toggleAria')} hideTitle={t('chat.thinking.hide')} showTitle={t('chat.thinking.show')} />
  <ViewToggle pref="showMessageUsage" label={t('chat.usage.toggle')} aria={t('chat.usage.toggleAria')} hideTitle={t('chat.usage.hide')} showTitle={t('chat.usage.show')} />
  <StatsPopover {session} />
</div>

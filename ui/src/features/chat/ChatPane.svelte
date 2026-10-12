<script lang="ts">
  import { pendingApproval } from '$lib/chat/approval';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ApprovalCard from './ApprovalCard.svelte';
  import AskDialog from './AskDialog.svelte';
  import BashConfirmDialog from './BashConfirmDialog.svelte';
  import ChatHeader from './ChatHeader.svelte';
  import Composer from './Composer.svelte';
  import CrashCard from './CrashCard.svelte';
  import MessageList from './MessageList.svelte';
  import PromptErrorBanner from './PromptErrorBanner.svelte';
  import UiRequestDialog from './UiRequestDialog.svelte';
  import WidgetStrip from './WidgetStrip.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let restarting = $state(false);

  const chat = $derived(session.chat);
  const approval = $derived(pendingApproval(chat));

  async function restart(): Promise<void> {
    restarting = true;
    await agentSessions.restart(session.key, {});
  }
</script>

<div class="flex size-full min-h-0 flex-col">
  <ChatHeader {session} />
  <MessageList {session} />

  <span class="sr-only" role="status">{session.streaming ? t('chat.list.responding') : ''}</span>

  <WidgetStrip {chat} placement="above" />

  {#if chat.crash}
    <CrashCard crash={chat.crash} onrestart={restart} {restarting} />
  {/if}

  {#if chat.promptError}
    <PromptErrorBanner error={chat.promptError} canRetry={chat.lastUserText !== null} onretry={() => void session.retry()} />
  {/if}

  {#if approval}
    <ApprovalCard {session} prompt={approval} />
  {/if}

  <Composer {session} />
  <WidgetStrip {chat} placement="below" />
</div>

{#if session.dialog}
  {#key session.dialog.id}
    <UiRequestDialog {session} request={session.dialog} />
  {/key}
{/if}

{#if session.ui.ask}
  <AskDialog {session} kind={session.ui.ask} />
{/if}

{#if session.ui.confirmBash !== null}
  <BashConfirmDialog {session} command={session.ui.confirmBash} />
{/if}

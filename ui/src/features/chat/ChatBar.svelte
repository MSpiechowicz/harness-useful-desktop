<script lang="ts">
  import Plus from '@lucide/svelte/icons/plus';

  import Button from '$lib/components/Button.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { ICON } from '$lib/components/icon';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { newChat } from '../shell/actions';
  import RoutineChip from '../routines/RoutineChip.svelte';
  import ChatControls from './ChatControls.svelte';
  import ChatMenu from './ChatMenu.svelte';
  import PreviewBadge from './PreviewBadge.svelte';

  const session = $derived(agentSessions.active);
  // The open chat names the page; with the rail collapsed it is the only place its name shows.
  const heading = $derived(agentSessions.activeKey ? agentSessions.title(agentSessions.activeKey) : t('chat.title'));
  const runnerId = $derived(session?.info.harness ?? agentSessions.activeStarting?.harness ?? null);
  const runnerSummary = $derived(runnerId ? harness.get(runnerId) : undefined);
  const runner = $derived(runnerSummary?.info);
</script>

<!-- Chat's header, rendered into the shell's top bar like every view's: the harness running the open chat, its name, and
     its controls. The open chats live in the rail, under Chat; a new one starts from the button here or with Ctrl+N. -->
<ViewHeader title={heading} leading={runner ? badge : undefined} center={session?.routine ? routine : undefined}>
  {#snippet actions()}
    <Button variant="primary" onclick={() => void newChat()}>
      {#snippet leading()}<Plus size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('chat.bar.newChat')}
      {#snippet trailing()}<Kbd keys="mod+n" small />{/snippet}
    </Button>
    {#if session}<ChatControls {session} />{/if}
  {/snippet}

  {#snippet fixed()}
    <ChatMenu session={session ?? undefined} />
  {/snippet}
</ViewHeader>

{#snippet routine()}
  {#if session?.routine}<RoutineChip routine={session.routine} />{/if}
{/snippet}

{#snippet badge()}
  {#if runner}<HarnessBadge info={runner} size="bar" />{/if}
  {#if runner && runnerSummary?.capabilities.chatPreview}<PreviewBadge name={runner.name} />{/if}
{/snippet}

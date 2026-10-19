<script lang="ts">
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import RoutineChip from '../routines/RoutineChip.svelte';
  import ChatControls from './ChatControls.svelte';

  const session = $derived(agentSessions.active);
  // The open chat names the page; with the rail collapsed it is the only place its name shows.
  const heading = $derived(agentSessions.activeKey ? agentSessions.title(agentSessions.activeKey) : t('chat.title'));
  const runnerId = $derived(session?.info.harness ?? agentSessions.activeStarting?.harness ?? null);
  const runner = $derived(runnerId ? harness.get(runnerId)?.info : undefined);
</script>

<!-- Chat's header, rendered into the shell's top bar like every view's: the harness running the open chat, its name, and
     its controls. The open chats live in the rail, under Chat; a new one starts with Ctrl+N. -->
<ViewHeader title={heading} leading={runner ? badge : undefined} center={session?.routine ? routine : undefined}>
  {#snippet actions()}
    {#if session}<ChatControls {session} />{/if}
  {/snippet}
</ViewHeader>

{#snippet routine()}
  {#if session?.routine}<RoutineChip routine={session.routine} />{/if}
{/snippet}

{#snippet badge()}
  {#if runner}<HarnessBadge info={runner} size="bar" />{/if}
{/snippet}

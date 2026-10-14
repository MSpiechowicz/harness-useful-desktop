<script lang="ts">
  import { fade } from 'svelte/transition';

  import { portal, SHELL_HEADER_SLOT } from '$lib/actions/portal';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { scaled } from '$lib/design/motion';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import ChatControls from './ChatControls.svelte';

  const session = $derived(agentSessions.active);
  // The open chat names the page; with the rail collapsed it is the only place its name shows.
  const heading = $derived(agentSessions.activeKey ? agentSessions.title(agentSessions.activeKey) : t('chat.title'));
  const runnerId = $derived(session?.info.harness ?? agentSessions.activeStarting?.harness ?? null);
  const runner = $derived(runnerId ? harness.get(runnerId)?.info : undefined);
</script>

<!-- Chat's header, rendered into the shell's top bar like every view's: the harness running the open chat, its name, and
     its controls. The open chats live in the rail, under Chat; a new one starts with Ctrl+N. -->
<header
  data-view-header
  use:portal={SHELL_HEADER_SLOT}
  in:fade|global={{ duration: scaled(160), delay: 80 }}
  out:fade|global={{ duration: scaled(100) }}
  class="flex h-[52px] min-w-0 items-center gap-3 px-2"
>
  {#if runner}<HarnessBadge info={runner} size="sm" />{/if}
  <h1 class="min-w-0 flex-1 truncate text-base leading-5 font-medium text-fg">{heading}</h1>

  {#if session}<ChatControls {session} />{/if}
</header>

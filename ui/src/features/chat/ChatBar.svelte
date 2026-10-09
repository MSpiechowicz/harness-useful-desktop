<script lang="ts">
  import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
  import { fade } from 'svelte/transition';

  import { portal, SHELL_HEADER_SLOT } from '$lib/actions/portal';
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import PaletteButton from '$lib/components/PaletteButton.svelte';
  import { scaled } from '$lib/design/motion';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { newChat } from '../shell/actions';
  import SessionTabs from './SessionTabs.svelte';

  const hasChats = $derived(agentSessions.sessions.length > 0);
  const ready = $derived(Boolean(harness.active?.detection.installed && project.active));
</script>

<!-- Chat's header, rendered into the shell's top bar like every view's: the chat tabs are the title once a chat
     exists; before that the view names itself. -->
<header
  data-view-header
  use:portal={SHELL_HEADER_SLOT}
  in:fade|global={{ duration: scaled(160), delay: 80 }}
  out:fade|global={{ duration: scaled(100) }}
  class="flex h-11 min-w-0 items-center gap-3 px-2"
>
  <h1 class={hasChats ? 'sr-only' : 'flex-1 text-base leading-5 font-medium text-fg'}>Chat</h1>

  {#if hasChats}
    <SessionTabs />
  {/if}

  <div class="flex shrink-0 items-center gap-2">
    <PaletteButton />
    <Button variant="primary" size="sm" disabled={!ready} onclick={() => void newChat()}>
      {#snippet leading()}<MessageSquarePlus size={ICON.sm} aria-hidden="true" />{/snippet}
      <span class="max-[1100px]:sr-only">New chat</span>
      <span class="max-[1100px]:hidden"><Kbd keys="mod+n" /></span>
    </Button>
  </div>
</header>

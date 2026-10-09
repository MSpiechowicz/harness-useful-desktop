<script lang="ts">
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { newChat } from '../shell/actions';
  import SessionTabs from './SessionTabs.svelte';

  const hasChats = $derived(agentSessions.sessions.length > 0);
  const ready = $derived(Boolean(harness.active?.detection.installed && project.active));
  const changed = $derived(git.status?.isRepo ? git.status.entries.length : 0);
</script>

<!-- Chat's header is one slim row: the chat tabs are the title, and the breadcrumb above already says where you are. -->
<header data-view-header class="sticky top-0 z-20 shrink-0 border-b border-line bg-surface-1/80 backdrop-blur-xl">
  <div class="page-gutter flex h-10 items-center gap-3">
    <h1 class={hasChats ? 'sr-only' : 'flex-1 text-base font-medium text-fg'}>Chat</h1>

    {#if hasChats}
      <SessionTabs />
    {/if}

    <div class="flex shrink-0 items-center gap-2">
      <Button variant="ghost" size="sm" disabled={!project.active} onclick={() => nav.go('changes')}>
        {#snippet leading()}<GitCompare size={ICON.sm} aria-hidden="true" />{/snippet}
        <span class="max-[1100px]:sr-only">View changes</span>
        {#if changed > 0}<span class="tabular rounded-full bg-surface-3 px-1.5 text-xs leading-5 text-fg-2">{changed}</span>{/if}
      </Button>

      <Button variant="primary" size="sm" disabled={!ready} onclick={() => void newChat()}>
        {#snippet leading()}<MessageSquarePlus size={ICON.sm} aria-hidden="true" />{/snippet}
        <span class="max-[1100px]:sr-only">New chat</span>
        <span class="max-[1100px]:hidden"><Kbd keys="mod+n" /></span>
      </Button>
    </div>
  </div>
</header>

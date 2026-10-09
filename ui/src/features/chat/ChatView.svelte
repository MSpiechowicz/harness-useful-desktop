<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
  import PackageOpen from '@lucide/svelte/icons/package-open';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { onboarding } from '../onboarding/onboarding.svelte';
  import { newChat } from '../shell/actions';
  import { count, scopeLabel } from '../shell/scope';
  import ChatPane from './ChatPane.svelte';
  import SessionTabs from './SessionTabs.svelte';

  const active = $derived(harness.active);
  const session = $derived(agentSessions.active);
  const ready = $derived(Boolean(active?.detection.installed && project.active));
  const changed = $derived(git.status?.isRepo ? git.status.entries.length : 0);
  let starting = $state(false);

  async function start(): Promise<void> {
    if (!active || !project.active) {
      return;
    }

    starting = true;
    await agentSessions.create(active.info.id, project.active.path);
    starting = false;
  }
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title="Chat" icon={MessageSquare} context={scopeLabel(agentSessions.sessions.length > 0 ? count(agentSessions.sessions.length, 'chat') : null)}>
    {#snippet actions()}
      <Button variant="ghost" disabled={!project.active} onclick={() => nav.go('changes')}>
        {#snippet leading()}<GitCompare size={ICON.sm} aria-hidden="true" />{/snippet}
        <span class="max-[1100px]:sr-only">View changes</span>
        {#if changed > 0}<span class="tabular rounded-full bg-surface-3 px-1.5 text-xs leading-5 text-fg-2">{changed}</span>{/if}
      </Button>

      <Button variant="primary" disabled={!ready} onclick={() => void newChat()}>
        {#snippet leading()}<MessageSquarePlus size={ICON.sm} aria-hidden="true" />{/snippet}
        <span class="max-[1100px]:sr-only">New chat</span>
        <span class="max-[1100px]:hidden"><Kbd keys="mod+n" /></span>
      </Button>
    {/snippet}

    {#snippet center()}
      {#if agentSessions.sessions.length > 0}<SessionTabs />{/if}
    {/snippet}
  </ViewHeader>

  <div class="min-h-0 flex-1">
    {#if harness.status === 'idle' || harness.status === 'loading'}
      <div class="flex flex-col gap-3 p-8" role="status" aria-label="Loading chat">
        <Skeleton width="14rem" height="1.75rem" />
        <Skeleton height="6rem" />
      </div>
    {:else if agentSessions.sessions.length === 0}
      <div class="grid size-full place-items-center">
        {#if !active?.detection.installed}
          <EmptyState icon={PackageOpen} title="Install a harness to start chatting" body="Harness Desktop talks to OMP or PI. Run the setup to detect the one you use.">
            {#snippet action()}
              <Button variant="primary" onclick={() => onboarding.restart()}>Set up a harness</Button>
            {/snippet}
          </EmptyState>
        {:else if !project.active}
          <EmptyState icon={FolderOpen} title="Open a project to chat" body="The agent works inside one folder. Pick the project you want it to work on.">
            {#snippet action()}
              <Button variant="primary" onclick={() => void project.pick()}>Open a project…</Button>
            {/snippet}
          </EmptyState>
        {:else}
          <EmptyState icon={MessageSquare} title="Talk to {active.info.name}" body="Start a chat in {project.active.name}. Responses stream in, and every tool call shows up as a card you can open.">
            {#snippet action()}
              <Button variant="primary" loading={starting} disabled={!ready} onclick={start}>Start a chat</Button>
            {/snippet}
          </EmptyState>
        {/if}
      </div>
    {:else}
      <div class="flex size-full min-h-0 flex-col">
        {#if session}
          {#key session.key}
            <ChatPane {session} />
          {/key}
        {/if}
      </div>
    {/if}
  </div>
</div>

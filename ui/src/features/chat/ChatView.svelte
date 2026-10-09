<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import PackageOpen from '@lucide/svelte/icons/package-open';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { onboarding } from '../onboarding/onboarding.svelte';
  import ChatBar from './ChatBar.svelte';
  import ChatPane from './ChatPane.svelte';

  const active = $derived(harness.active);
  const session = $derived(agentSessions.active);
  const ready = $derived(Boolean(active?.detection.installed && project.active));
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
  <ChatBar />

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

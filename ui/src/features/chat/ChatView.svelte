<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import PackageOpen from '@lucide/svelte/icons/package-open';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { restoreWorkspace } from '$lib/stores/workspace.svelte';

  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import ChatBar from './ChatBar.svelte';
  import ChatPane from './ChatPane.svelte';

  const active = $derived(harness.active);
  const session = $derived(agentSessions.active);
  const ready = $derived(Boolean(active?.detection.installed && project.active));
  const target = $derived(ready && active && project.active ? `${active.info.id}\n${project.active.path}` : null);
  const starting = $derived(agentSessions.starting.length > 0);
  // The target whose automatic start failed, so a broken harness shows a retry instead of looping.
  let failed = $state<string | null>(null);

  /** The launch's first start reopens the last run's chats instead, when there are any. */
  let restoring = false;

  async function start(): Promise<void> {
    if (!active || !project.active || !target || restoring) {
      return;
    }

    restoring = true;
    const restored = await restoreWorkspace(agentSessions);
    restoring = false;
    if (restored || agentSessions.sessions.length > 0) {
      return;
    }

    const attempt = target;
    const session = await agentSessions.create(active.info.id, project.active.path);
    failed = session ? null : attempt;
  }

  // Opening the app always means chatting: start one as soon as a harness and a project are ready.
  $effect(() => {
    if (harness.status === 'ready' && agentSessions.sessions.length === 0 && target && !starting && failed !== target) {
      void start();
    }
  });
</script>

<!-- The agent can take a few seconds to start (longer right after a plugin update), so it says what it is doing. -->
{#snippet startingChat(harnessName: string, projectName: string)}
  <div class="flex flex-col items-center gap-3 text-center" role="status" aria-label="Starting chat">
    <Spinner size={20} class="text-accent-text" />
    <p class="text-sm text-fg-2">Starting {harnessName} in <span class="text-fg">{projectName}</span>…</p>
  </div>
{/snippet}

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
        {:else if failed === target && !starting}
          <EmptyState icon={MessageSquare} title="{active.info.name} did not start" body="The chat in {project.active.name} could not start. Check the notification for details, then try again.">
            {#snippet action()}
              <Button variant="primary" onclick={start}>Try again</Button>
            {/snippet}
          </EmptyState>
        {:else}
          {@render startingChat(active.info.name, project.active.name)}
        {/if}
      </div>
    {:else}
      <div class="flex size-full min-h-0 flex-col">
        {#if session}
          {#key session.key}
            <ChatPane {session} />
          {/key}
        {:else if agentSessions.activeStarting}
          {@const chat = agentSessions.activeStarting}
          <div class="grid size-full place-items-center">
            {@render startingChat(harness.get(chat.harness)?.info.name ?? 'the agent', chat.project.split(/[\\/]/).filter(Boolean).at(-1) ?? chat.project)}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

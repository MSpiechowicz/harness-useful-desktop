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
  import { t } from '$lib/i18n/i18n.svelte';

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

  /** Stands in for the project's name so the sentence can be split around it and the name styled on its own. */
  const PROJECT_MARK = '\u0000';

  function startingParts(harnessName: string | null): string[] {
    const sentence = harnessName ? t('chat.view.starting', { harness: harnessName, project: PROJECT_MARK }) : t('chat.view.startingAgent', { project: PROJECT_MARK });

    return sentence.split(PROJECT_MARK);
  }

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
{#snippet startingChat(harnessName: string | null, projectName: string)}
  {@const [before, after] = startingParts(harnessName)}
  <div class="flex flex-col items-center gap-3 text-center" role="status" aria-label={t('chat.view.starting.label')}>
    <Spinner size={20} class="text-accent-text" />
    <p class="text-sm text-fg-2">{before}<span class="text-fg">{projectName}</span>{after}</p>
  </div>
{/snippet}

<div class="flex size-full min-h-0 flex-col">
  <ChatBar />

  <div class="min-h-0 flex-1">
    {#if harness.status === 'idle' || harness.status === 'loading'}
      <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('chat.view.loading')}>
        <Skeleton width="14rem" height="1.75rem" />
        <Skeleton height="6rem" />
      </div>
    {:else if agentSessions.sessions.length === 0}
      <div class="grid size-full place-items-center">
        {#if !active?.detection.installed}
          <EmptyState icon={PackageOpen} title={t('chat.view.install.title')} body={t('chat.view.install.body')}>
            {#snippet action()}
              <Button variant="primary" onclick={() => onboarding.restart()}>{t('chat.view.install.action')}</Button>
            {/snippet}
          </EmptyState>
        {:else if !project.active}
          <EmptyState icon={FolderOpen} title={t('chat.view.project.title')} body={t('chat.view.project.body')}>
            {#snippet action()}
              <Button variant="primary" onclick={() => void project.pick()}>{t('chat.view.project.action')}</Button>
            {/snippet}
          </EmptyState>
        {:else if failed === target && !starting}
          <EmptyState icon={MessageSquare} title={t('chat.view.failed.title', { harness: active.info.name })} body={t('chat.view.failed.body', { project: project.active.name })}>
            {#snippet action()}
              <Button variant="primary" onclick={start}>{t('chat.view.failed.action')}</Button>
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
            {@render startingChat(harness.get(chat.harness)?.info.name ?? null, chat.project.split(/[\\/]/).filter(Boolean).at(-1) ?? chat.project)}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

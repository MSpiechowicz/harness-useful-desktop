<script lang="ts">
  import { untrack } from 'svelte';

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
  import { nav } from '$lib/stores/nav.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { restoreWorkspace } from '$lib/stores/workspace.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { joinNames } from '$lib/utils/format';

  import ChatBar from './ChatBar.svelte';
  import ChatPane from './ChatPane.svelte';
  import ChatSoon from './ChatSoon.svelte';

  // Chats go to a harness that can chat, which is not always the one the other views show.
  const active = $derived(harness.chatActive);
  const comingSoon = $derived(harness.chatComingSoon);
  const session = $derived(agentSessions.active);
  // The harnesses the app can chat with, named from their capabilities, so one that gains chat appears here by itself.
  // Until the list names one (it failed to load), the hint names none rather than "Chat needs ."
  const chatHarnesses = $derived(harness.summaries.filter((summary) => summary.capabilities.chat).map((summary) => summary.info.name));
  const installBody = $derived(chatHarnesses.length > 0 ? t('chat.view.install.body', { harnesses: joinNames(chatHarnesses, 'disjunction') }) : undefined);
  const ready = $derived(Boolean(active?.detection.installed && project.active));
  const target = $derived(ready && active && project.active ? `${active.info.id}\n${project.active.path}` : null);
  const starting = $derived(agentSessions.starting.length > 0);
  // The target whose automatic start failed, so a broken harness shows a retry instead of looping.
  let failed = $state<string | null>(null);

  /** Without an installed harness there is no chat; Settings is where one is set up. */
  function openHarnessSettings(): void {
    viewState.settingsTarget = 'harnesses';
    nav.go('settings');
  }

  /** The launch's first start reopens the last run's chats instead, when there are any. */
  let restoring = $state(false);

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

  // Opening the app always means chatting: start one as soon as a harness and a project are ready. `start` runs untracked:
  // the `restoring` flag it reads and sets must not run this effect again, or an answer that comes back at once loops forever.
  $effect(() => {
    if (harness.status === 'ready' && agentSessions.sessions.length === 0 && target && !starting && failed !== target) {
      untrack(() => void start());
    }
  });
</script>

<!-- The agent can take a few seconds to start (longer right after a plugin update), so it says what it is doing. -->
{#snippet startingChat(harnessName: string | null, projectName: string)}
  <div class="flex flex-col items-center gap-4 text-center" role="status" aria-label={t('chat.view.starting.label')}>
    <Spinner size={32} class="text-accent-text" />
    <div class="flex flex-col gap-1">
      <p class="text-base text-fg">{restoring ? t('chat.view.restoring') : harnessName ? t('chat.view.starting', { harness: harnessName }) : t('chat.view.startingAgent')}</p>
      <p class="text-sm text-fg-3">{t('chat.view.startingProject', { project: projectName })}</p>
    </div>
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
        {#if comingSoon}
          <ChatSoon name={comingSoon.info.name} />
        {:else if !active?.detection.installed}
          <EmptyState icon={PackageOpen} title={t('chat.view.install.title')} body={installBody}>
            {#snippet action()}
              <Button variant="primary" onclick={openHarnessSettings}>{t('chat.view.install.action')}</Button>
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

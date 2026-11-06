<script lang="ts">
  import FileCode from '@lucide/svelte/icons/file-code';
  import NotebookText from '@lucide/svelte/icons/notebook-text';

  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { type GuardKind, offReason, projectIsOn } from '$lib/stores/memory-chat';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { cx } from '$lib/utils/cx';

  import MemoryGuardRow from './MemoryGuardRow.svelte';

  interface Props {
    kind: GuardKind;
    /** The chat's agent. */
    agentKey: string;
    /** The saving root of the chat's project, as the global guards list it; null while it is not known. */
    root: string | null;
    /** The folder the chat works in: turning the project on names it, so its key is the one the chat saves under (a git folder alone has another). */
    folder: string;
    /** What the project is called for people. */
    repoName: string;
  }

  let { kind, agentKey, root, folder, repoName }: Props = $props();

  const KINDS = {
    notes: { icon: NotebookText, title: 'memory.guards.notes', chatOn: 'memory.guards.chat.on.notes', chatOff: 'memory.guards.chat.off.notes', repoOff: 'memory.guards.repo.off.notes' },
    scripts: { icon: FileCode, title: 'memory.guards.scripts', chatOn: 'memory.guards.chat.on.scripts', chatOff: 'memory.guards.chat.off.scripts', repoOff: 'memory.guards.repo.off.scripts' },
  } as const;

  const look = $derived(KINDS[kind]);
  const guards = $derived(memoryChat.get(agentKey)?.guards ?? null);
  const reason = $derived(guards ? offReason(kind, guards, memoryChat.guards, root) : null);
  const everyProject = $derived(memoryChat.guards[kind].everyProject);
  const projectOn = $derived(projectIsOn(memoryChat.guards[kind], root));
  const repoIsOff = $derived(reason === 'repo');
  const everywhereOff = $derived(reason === 'global');
  const title = $derived(t(look.title));

  const repoDescription = $derived.by(() => {
    if (everyProject) {
      return t('memory.guards.repo.every');
    }

    return t(projectOn ? 'memory.guards.repo.on' : look.repoOff, { repo: repoName });
  });

  const chatDescription = $derived.by(() => {
    if (everywhereOff) {
      return t('memory.guards.chat.global');
    }

    if (repoIsOff) {
      return t('memory.guards.chat.repoOff', { repo: repoName });
    }

    return t(guards?.[kind].on ? look.chatOn : look.chatOff);
  });
</script>

{#if guards}
  <!-- A kind in its own inset card: what is saved in this chat, in this repository, and whether each save waits for a click. -->
  <section class={cx('chat-radius overflow-hidden border bg-surface-1', guards.paused ? 'border-warning/35' : 'border-line')} aria-label={title}>
    <h3 class="flex items-center gap-2 px-3 py-2.5 text-sm font-semibold text-fg">
      <look.icon size={ICON.sm} class="text-fg-2" aria-hidden="true" />
      {title}
      {#if guards.paused}<span class="text-xs font-medium text-warning">{t('memory.guards.paused')}</span>{/if}
    </h3>

    <div class={cx('flex flex-col transition-opacity duration-[var(--dur-fast)]', guards.paused && 'opacity-55')}>
      <MemoryGuardRow
        title={t('memory.guards.chat')}
        description={chatDescription}
        label={`${title}: ${t('memory.guards.chat')}`}
        checked={reason === null}
        disabled={repoIsOff || everywhereOff}
        dim={repoIsOff || everywhereOff}
        onchange={(on) => void memoryChat.setKind(agentKey, kind, { on })}
      />

      {#if root !== null}
        <MemoryGuardRow
          title={t('memory.guards.repo')}
          description={repoDescription}
          strong={repoName}
          label={`${title}: ${t('memory.guards.repo')}`}
          checked={projectOn}
          disabled={everyProject}
          onchange={(on) => void memoryChat.setRepoOn(kind, on ? folder : root, on)}
        />
      {/if}

      <MemoryGuardRow
        title={t('memory.guards.ask')}
        description={t(guards[kind].ask ? 'memory.guards.ask.on' : 'memory.guards.ask.off')}
        label={`${title}: ${t('memory.guards.ask')}`}
        checked={guards[kind].ask && reason === null}
        disabled={reason !== null}
        dim={reason !== null}
        onchange={(ask) => void memoryChat.setKind(agentKey, kind, { ask })}
      />
    </div>
  </section>
{/if}

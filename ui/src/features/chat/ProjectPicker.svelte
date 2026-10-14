<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitBranch from '@lucide/svelte/icons/git-branch';

  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { ipc } from '$lib/ipc/client';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';
  import { t } from '$lib/i18n/i18n.svelte';

  import { switchProject, switchToPickedProject } from '../shell/actions';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let open = $state(false);
  /** The checked-out branch of each project, by path (`null`: not a repository or a detached HEAD). */
  let branches = $state<Record<string, string | null>>({});

  async function readBranch(target: string): Promise<void> {
    try {
      const status = await ipc.invoke('git_status', { project: target });
      branches = { ...branches, [target]: status.isRepo ? (status.branch ?? (status.head ? status.head.slice(0, 7) : null)) : null };
    } catch {
      // A branch is a hint; a project git cannot read simply shows none.
    }
  }

  // Opening the menu reads every listed project's branch, so the list is current.
  $effect(() => {
    if (open) {
      for (const recent of others) {
        if (recent.isGitRepo) {
          void readBranch(recent.path);
        }
      }
    }
  });

  const path = $derived(session.info.project);
  const name = $derived(path.split(/[\\/]/).filter(Boolean).at(-1) ?? path);
  const others = $derived(project.recents.filter((recent) => recent.exists));
  /** A chat that moves with its project (OMP) cannot move while the agent works. */
  const locked = $derived(session.streaming && Boolean(harness.get(session.info.harness)?.capabilities.moveSession));

  const ROW = 'flex w-full items-center gap-2.5 rounded-sm px-2.5 py-1.5 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press focus-visible:bg-hover';

  function choose(target: string, close: () => void): void {
    close();
    if (target !== path) {
      void switchProject(target);
    }
  }
</script>

<!-- The chat's project, and a quick way to work in another one: its open chat, or a new chat there. -->
<Popover label={t('chat.project.popover')} kind="menu" side="top" align="end" bind:open class="w-max max-w-md!" rootClass="max-w-[40%] shrink">
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      title={locked ? t('chat.project.locked') : path}
      disabled={locked}
      aria-label={t('chat.project.aria', { name })}
      class="inline-flex h-control max-w-full min-w-0 items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-transparent aria-expanded:bg-hover aria-expanded:text-fg"
    >
      <Folder size={14} aria-hidden="true" class="shrink-0" />
      <span class="min-w-0 truncate @max-[460px]:hidden">{name}</span>
      <ChevronDown size={16} aria-hidden="true" class="shrink-0 text-fg-3 @max-[460px]:hidden" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="flex max-h-72 flex-col gap-0.5 overflow-y-auto" role="group" aria-label={t('chat.project.recent')}>
      {#each others as recent (recent.path)}
        {@const current = recent.path === path}
        {@const Icon = recent.isGitRepo ? FolderGit2 : Folder}
        <button type="button" role="menuitemradio" aria-checked={current} title={recent.path} data-menu-item class={cx(ROW, current ? 'text-fg' : 'text-fg-2')} onclick={() => choose(recent.path, close)}>
          <Icon size={16} aria-hidden="true" class={cx('shrink-0', current ? 'text-accent-text' : 'text-fg-3')} />
          <span class="min-w-0 flex-1 truncate">{recent.name}</span>
          {#if branches[recent.path]}
            <span class="flex max-w-56 min-w-0 items-center gap-1 text-xs text-fg-3">
              <GitBranch size={12} aria-hidden="true" class="shrink-0" />
              <span class="truncate">{branches[recent.path]}</span>
            </span>
          {/if}
          <Check size={16} aria-hidden="true" class={cx('shrink-0 text-accent-text', !current && 'invisible')} />
        </button>
      {/each}
    </div>

    <div class="my-1.5 border-t border-line" role="separator"></div>

    <button
      type="button"
      data-menu-item
      class={cx(ROW, 'text-fg-2')}
      onclick={() => {
        close();
        void switchToPickedProject();
      }}
    >
      <FolderOpen size={16} aria-hidden="true" class="shrink-0 text-fg-3" />
      {t('chat.project.open')}
    </button>
  {/snippet}
</Popover>

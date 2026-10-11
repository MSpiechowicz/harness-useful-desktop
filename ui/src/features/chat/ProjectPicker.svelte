<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { switchProject, switchToPickedProject } from '../shell/actions';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let open = $state(false);

  const path = $derived(session.info.project);
  const name = $derived(path.split(/[\\/]/).filter(Boolean).at(-1) ?? path);
  const others = $derived(project.recents.filter((recent) => recent.exists));

  const ROW = 'flex w-full items-center gap-2.5 rounded-sm px-2.5 py-1.5 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press focus-visible:bg-hover';

  function choose(target: string, close: () => void): void {
    close();
    if (target !== path) {
      void switchProject(target);
    }
  }
</script>

<!-- The chat's project, and a quick way to work in another one: its open chat, or a new chat there. -->
<Popover label="Switch project" kind="menu" side="top" align="end" bind:open class="w-max max-w-md!">
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      title={path}
      aria-label="Project: {name}. Switch project"
      class="inline-flex h-8 min-w-0 items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg"
    >
      <Folder size={14} aria-hidden="true" class="shrink-0" />
      <span class="truncate">{name}</span>
      <ChevronDown size={16} aria-hidden="true" class="shrink-0 text-fg-3" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="flex max-h-72 flex-col gap-0.5 overflow-y-auto" role="group" aria-label="Recent projects">
      {#each others as recent (recent.path)}
        {@const current = recent.path === path}
        {@const Icon = recent.isGitRepo ? FolderGit2 : Folder}
        <button type="button" role="menuitemradio" aria-checked={current} title={recent.path} data-menu-item class={cx(ROW, current ? 'text-fg' : 'text-fg-2')} onclick={() => choose(recent.path, close)}>
          <Icon size={16} aria-hidden="true" class={cx('shrink-0', current ? 'text-accent-text' : 'text-fg-3')} />
          <span class="min-w-0 flex-1 truncate">{recent.name}</span>
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
      Open folder…
    </button>
  {/snippet}
</Popover>

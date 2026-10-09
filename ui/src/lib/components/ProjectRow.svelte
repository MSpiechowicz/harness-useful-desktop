<script lang="ts">
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';

  import Badge from '$lib/components/Badge.svelte';
  import type { ProjectInfo } from '$lib/ipc/bindings/ProjectInfo';
  import { cx } from '$lib/utils/cx';

  interface Props {
    project: ProjectInfo;
    active: boolean;
    onselect: () => void;
    /** Mark the row as an item of the surrounding menu or popover, for arrow-key navigation. */
    menuItem?: boolean;
  }

  let { project, active, onselect, menuItem = false }: Props = $props();

  const Icon = $derived(project.isGitRepo ? FolderGit2 : Folder);
</script>

<button
  type="button"
  aria-current={active ? 'true' : undefined}
  aria-disabled={!project.exists}
  data-menu-item={menuItem ? '' : undefined}
  onclick={() => project.exists && onselect()}
  class={cx(
    'flex w-full items-center gap-3 rounded-md border px-3.5 py-2.5 text-left transition-colors duration-[var(--dur-fast)]',
    active ? 'border-accent/50 bg-accent-wash' : 'border-line bg-glass hover:bg-hover',
    !project.exists && 'cursor-not-allowed opacity-60',
  )}
>
  <Icon class={active ? 'text-accent-text' : 'text-fg-3'} size={18} aria-hidden="true" />
  <span class="flex min-w-0 flex-1 flex-col">
    <span class="truncate text-base font-medium text-fg">{project.name}</span>
    <span class="truncate font-mono text-xs text-fg-3">{project.path}</span>
  </span>
  {#if !project.exists}
    <Badge tone="warning">Missing</Badge>
  {:else if active}
    <Badge tone="accent">Open</Badge>
  {/if}
</button>

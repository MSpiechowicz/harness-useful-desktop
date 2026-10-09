<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import FileCog from '@lucide/svelte/icons/file-cog';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderPlus from '@lucide/svelte/icons/folder-plus';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import History from '@lucide/svelte/icons/history';
  import MessageSquare from '@lucide/svelte/icons/message-square';

  import Badge from '$lib/components/Badge.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';

  /** What opening a project turns on, as one quiet line under the picker. */
  const ENABLES: { icon: IconComponent; label: string }[] = [
    { icon: MessageSquare, label: 'Chat' },
    { icon: GitCompare, label: 'Changes' },
    { icon: History, label: 'Sessions' },
    { icon: FileCog, label: 'Project settings' },
  ];

  const ROW = 'flex w-full items-center gap-4 px-4 py-3 text-left transition-colors duration-[var(--dur-fast)]';
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow="Your project"
    title="Pick a project to work in"
    body="The agent works inside one folder. Optional: you can open or change it any time from the top bar."
  />

  <!-- One picker: remembered projects as rows, with opening another folder as the last row of the same list. -->
  <div class="overflow-hidden rounded-lg border border-line bg-glass shadow-float">
    {#if project.recents.length > 0}
      <ul class="max-h-64 divide-y divide-line overflow-y-auto [@media(max-height:720px)]:max-h-44" aria-label="Recent projects">
        {#each project.recents as recent (recent.path)}
          {@const active = project.active?.path === recent.path}
          {@const Icon = recent.isGitRepo ? FolderGit2 : Folder}
          <li>
            <button
              type="button"
              aria-current={active ? 'true' : undefined}
              aria-disabled={!recent.exists}
              class={cx(
                ROW,
                'group',
                active ? 'bg-accent-wash' : 'hover:bg-hover active:bg-press',
                !recent.exists && 'cursor-not-allowed opacity-60',
              )}
              onclick={() => recent.exists && void project.open(recent.path)}
            >
              <span class={cx('grid size-9 shrink-0 place-items-center rounded-md', active ? 'bg-accent/20' : 'bg-surface-2')}>
                <Icon class={active ? 'text-accent-text' : 'text-fg-2'} size={18} aria-hidden="true" />
              </span>

              <span class="flex min-w-0 flex-1 flex-col">
                <span class="flex items-center gap-2">
                  <span class="truncate text-base font-medium text-fg">{recent.name}</span>
                  {#if recent.isGitRepo}<Badge>git</Badge>{/if}
                </span>
                <span class="truncate font-mono text-xs text-fg-3">{recent.path}</span>
              </span>

              {#if !recent.exists}
                <Badge tone="warning">Missing</Badge>
              {:else if active}
                <span class="inline-flex items-center gap-1.5 text-sm font-medium text-accent-text"><Check size={16} aria-hidden="true" />Open</span>
              {:else}
                <ChevronRight class="text-fg-3 opacity-0 transition-opacity group-hover:opacity-100" size={18} aria-hidden="true" />
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <button
      type="button"
      class={cx(ROW, 'group hover:bg-hover active:bg-press', project.recents.length > 0 && 'border-t border-line')}
      onclick={() => void project.pick()}
    >
      <span class="grid size-9 shrink-0 place-items-center rounded-md border border-dashed border-line-strong">
        <FolderPlus class="text-fg-2 group-hover:text-accent-text" size={18} aria-hidden="true" />
      </span>
      <span class="flex min-w-0 flex-1 flex-col">
        <span class="text-base font-medium text-fg">{project.recents.length > 0 ? 'Open another folder…' : 'Open a folder…'}</span>
        <span class="text-xs text-fg-3">Any folder works; a git repository also unlocks Changes.</span>
      </span>
    </button>
  </div>

  <p class="flex flex-wrap items-center gap-2 text-sm text-fg-3">
    <span>A project enables</span>
    {#each ENABLES as item (item.label)}
      <span class="inline-flex items-center gap-1.5 rounded-full border border-line bg-surface-1 px-2.5 py-1 text-fg-2">
        <item.icon size={14} aria-hidden="true" />{item.label}
      </span>
    {/each}
  </p>
</div>

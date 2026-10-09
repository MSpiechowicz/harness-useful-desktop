<script lang="ts">
  import FileCog from '@lucide/svelte/icons/file-cog';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import History from '@lucide/svelte/icons/history';
  import MessageSquare from '@lucide/svelte/icons/message-square';

  import Button from '$lib/components/Button.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import ProjectRow from '$lib/components/ProjectRow.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';

  /** What opening a project changes in the app, so the choice is concrete. */
  const UNLOCKS: { icon: IconComponent; title: string; body: string }[] = [
    { icon: MessageSquare, title: 'Chat works in it', body: 'The agent starts in this folder and reads and edits its files.' },
    { icon: GitCompare, title: 'Changes shows its diff', body: 'Review what the agent changed, comment on lines, send the review back.' },
    { icon: History, title: 'Sessions are its history', body: 'Earlier chats for this folder, with their branches, ready to resume.' },
    { icon: FileCog, title: 'Project settings apply', body: 'Project-level config and extensions are read next to your global ones.' },
  ];
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow="Your project"
    title="Pick a project to work in"
    body="Optional: you can open or change a project any time from the top bar."
  />

  <div class="grid grid-cols-2 items-start gap-4">
    <section class="flex flex-col gap-3" aria-labelledby="recent-projects">
      <div class="flex items-center justify-between gap-3">
        <h3 id="recent-projects" class="text-sm font-medium tracking-wider text-fg-3 uppercase">Recent projects</h3>
        <Button variant="ghost" size="sm" onclick={() => void project.pick()}>
          {#snippet leading()}<FolderOpen size={16} aria-hidden="true" />{/snippet}
          Open folder…
        </Button>
      </div>

      {#if project.recents.length > 0}
        <ul class="flex max-h-72 flex-col gap-2 overflow-y-auto" aria-label="Recent projects">
          {#each project.recents as recent (recent.path)}
            <li>
              <ProjectRow project={recent} active={project.active?.path === recent.path} onselect={() => void project.open(recent.path)} />
            </li>
          {/each}
        </ul>
      {:else}
        <p class="rounded-lg border border-line bg-glass p-5 text-sm text-fg-2">No recent projects yet. Open a folder to start; it will be remembered here.</p>
      {/if}
    </section>

    <section class="flex flex-col gap-4 rounded-lg border border-line bg-glass p-5 backdrop-blur-md" aria-labelledby="project-unlocks">
      <h3 id="project-unlocks" class="text-sm font-medium tracking-wider text-fg-3 uppercase">What a project unlocks</h3>
      <ul class="flex flex-col gap-3.5">
        {#each UNLOCKS as item (item.title)}
          <li class="flex items-start gap-3">
            <span class="grid size-8 shrink-0 place-items-center rounded-md bg-accent-wash">
              <item.icon class="text-accent-text" size={16} aria-hidden="true" />
            </span>
            <div class="flex flex-col gap-0.5">
              <p class="text-sm font-medium text-fg">{item.title}</p>
              <p class="text-sm text-fg-2">{item.body}</p>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  </div>
</div>

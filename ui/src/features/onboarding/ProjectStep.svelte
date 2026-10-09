<script lang="ts">
  import FileCog from '@lucide/svelte/icons/file-cog';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import History from '@lucide/svelte/icons/history';
  import MessageSquare from '@lucide/svelte/icons/message-square';

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

  <!-- The action leads: a large folder tile beside the remembered projects; what a project unlocks runs underneath. -->
  <div class="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] items-stretch gap-4">
    <button
      type="button"
      class="group flex flex-col items-center justify-center gap-3 rounded-lg border-2 border-dashed border-line-strong p-6 text-center [@media(max-height:720px)]:gap-2 [@media(max-height:720px)]:p-4 transition-colors duration-[var(--dur-base)] hover:border-accent/60 hover:bg-accent-wash"
      onclick={() => void project.pick()}
    >
      <span class="grid size-12 place-items-center rounded-full bg-accent-wash transition-transform duration-[var(--dur-base)] group-hover:scale-105">
        <FolderOpen class="text-accent-text" size={22} aria-hidden="true" />
      </span>
      <span class="text-base font-medium text-fg">Open a folder…</span>
      <span class="text-sm text-fg-3">Any folder works; a git repository unlocks Changes.</span>
    </button>

    <section class="flex min-w-0 flex-col gap-3" aria-labelledby="recent-projects">
      <h3 id="recent-projects" class="text-sm font-medium tracking-wider text-fg-3 uppercase">Recent projects</h3>
      {#if project.recents.length > 0}
        <ul class="flex max-h-56 flex-col gap-2 overflow-y-auto" aria-label="Recent projects">
          {#each project.recents as recent (recent.path)}
            <li>
              <ProjectRow project={recent} active={project.active?.path === recent.path} onselect={() => void project.open(recent.path)} />
            </li>
          {/each}
        </ul>
      {:else}
        <p class="rounded-lg border border-line bg-glass p-5 text-sm text-fg-2">No recent projects yet. Folders you open are remembered here.</p>
      {/if}
    </section>
  </div>

  <section class="flex flex-col gap-3" aria-labelledby="project-unlocks">
    <h3 id="project-unlocks" class="text-sm font-medium tracking-wider text-fg-3 uppercase">What a project unlocks</h3>
    <ul class="grid grid-cols-4 gap-3">
      {#each UNLOCKS as item (item.title)}
        <li class="flex flex-col gap-2 border-t-2 border-accent/40 pt-3">
          <item.icon class="text-accent-text" size={18} aria-hidden="true" />
          <p class="text-sm font-medium text-fg">{item.title}</p>
          <p class="text-xs text-fg-2">{item.body}</p>
        </li>
      {/each}
    </ul>
  </section>
</div>

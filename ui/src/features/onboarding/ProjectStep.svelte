<script lang="ts">
  import Bot from '@lucide/svelte/icons/bot';
  import FileCog from '@lucide/svelte/icons/file-cog';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import History from '@lucide/svelte/icons/history';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Repeat from '@lucide/svelte/icons/repeat';

  import Button from '$lib/components/Button.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import { onboarding } from './onboarding.svelte';
  import StepHeader from './StepHeader.svelte';

  const current = $derived(project.active);
  /** Without a project yet, the remembered ones can be reopened in one click. */
  const reopen = $derived(project.recents.filter((recent) => recent.exists).slice(0, 4));

  // A live fact about the chosen project: its git state.
  $effect(() => {
    git.ensure(current?.path ?? null);
  });

  const driverName = $derived(onboarding.driver ? (harness.get(onboarding.driver)?.info.name ?? null) : null);
  const changed = $derived(git.activeProject === current?.path ? (git.status?.entries.length ?? null) : null);
  const branch = $derived(git.activeProject === current?.path ? (git.status?.branch ?? null) : null);

  function changesLine(): string {
    if (current && !current.isGitRepo) {
      return 'Needs git.';
    }

    if (changed === null) {
      return 'The agent’s diff.';
    }

    const where = branch ? ` on ${branch}` : '';

    return changed === 0 ? `Clean${where}.` : `${changed} changed${where}.`;
  }

  /** The app's core loop, taught while the folder it will run in is chosen. */
  const LOOP: { icon: IconComponent; title: string; body: string; keys?: string }[] = [
    { icon: MessageSquare, title: 'You ask', body: 'Describe the change in Chat.', keys: 'mod+n' },
    { icon: Bot, title: 'The agent works', body: 'It reads and edits files in this folder.' },
    { icon: GitCompare, title: 'You review', body: 'Comment on its diff in Changes and send it back.', keys: 'mod+3' },
    { icon: History, title: 'Pick up later', body: 'Every chat is kept in Sessions, ready to resume.', keys: 'mod+2' },
  ];

  /** What the open project turns on, with live facts where the app has them. */
  const tiles = $derived<{ icon: IconComponent; title: string; body: string; off?: boolean }[]>([
    { icon: MessageSquare, title: 'Chat', body: driverName ? `Ready with ${driverName}.` : 'Works in this folder.' },
    { icon: GitCompare, title: 'Changes', body: changesLine(), off: Boolean(current && !current.isGitRepo) },
    { icon: History, title: 'Sessions', body: 'Resume any chat.' },
    { icon: FileCog, title: 'Project settings', body: 'Read with global ones.' },
  ]);
</script>

<div class={cx('mx-auto flex w-full flex-col gap-5 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow="Your project"
    title="Pick a project to work in"
    body="The agent works inside one folder. Optional: you can open or change it any time from the top bar."
  />

  <!--
    One card is the whole choice: the project (or the way to open one) on top, and what it turns on along the bottom.
    A single crisp border with a soft shadow below; no second ring beside it.
  -->
  <section
    class={cx(
      'flex flex-col rounded-[16px] border p-[22px]',
      current
        ? 'edge-accent bg-surface-1 shadow-float'
        : 'border-dashed border-line-strong',
    )}
    aria-label={current ? 'Selected project' : 'No project yet'}
  >
    {#if current}
      {@const Icon = current.isGitRepo ? FolderGit2 : Folder}
      <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-[18px]">
        <span class="grid size-[64px] place-items-center rounded-[16px] bg-accent-fill text-on-accent">
          <Icon size={30} aria-hidden="true" />
        </span>
        <div class="flex min-w-0 flex-col gap-1">
          <p class="truncate text-[22px] leading-tight font-semibold tracking-tight text-fg">{current.name}</p>
          <p class="truncate font-mono text-xs text-fg-3" title={current.path}>{current.path}</p>
          <div class="mt-1 flex flex-wrap items-center gap-2">
            {#if current.isGitRepo}
              <span class="inline-flex h-6 items-center gap-1.5 rounded-full border border-line-strong bg-surface-1 px-2.5 text-xs text-fg-2"><GitBranch size={14} aria-hidden="true" />git repository</span>
            {:else}
              <span class="inline-flex h-6 items-center rounded-full border border-line-strong bg-surface-1 px-2.5 text-xs text-fg-3">no git</span>
            {/if}
            <span class="inline-flex h-6 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium text-success"><span class="size-1.5 rounded-full bg-success"></span>Selected</span>
          </div>
        </div>
        <Button onclick={() => void project.pick()}>Change…</Button>
      </div>
    {:else}
      <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-[18px]">
        <span class="grid size-[64px] place-items-center rounded-[16px] bg-accent-wash text-accent-text">
          <FolderOpen size={30} aria-hidden="true" />
        </span>
        <div class="flex min-w-0 flex-col gap-1">
          <p class="text-[22px] leading-tight font-semibold tracking-tight text-fg">No project yet</p>
          <p class="text-sm text-fg-3">Any folder works; a git repository also unlocks Changes.</p>
          {#if reopen.length > 0}
            <div class="mt-1 flex flex-wrap items-center gap-2 text-xs text-fg-3">
              <span>Or reopen</span>
              {#each reopen as recent (recent.path)}
                <button
                  type="button"
                  class="inline-flex h-6 max-w-48 items-center gap-1.5 rounded-full border border-line-strong bg-surface-1 px-2.5 text-xs text-fg-2 transition-colors hover:border-accent/50 hover:text-fg"
                  title={recent.path}
                  onclick={() => void project.open(recent.path)}
                >
                  <Folder size={12} aria-hidden="true" /><span class="truncate">{recent.name}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
        <Button variant="primary" onclick={() => void project.pick()}>Open a folder…</Button>
      </div>
    {/if}

    <!-- Four equal cells with thin dividers, so the spacing reads the same whatever the text length. -->
    <ul class="mt-[22px] grid grid-cols-4 divide-x divide-line border-t border-line pt-[18px]" aria-label="In this project">
      {#each tiles as tile (tile.title)}
        <li class={cx('flex items-start gap-3 px-4 first:pl-0 last:pr-0', (tile.off || !current) && 'opacity-55')}>
          <span class={cx('grid size-8 shrink-0 place-items-center rounded-md', tile.off ? 'bg-surface-2' : 'bg-accent-wash')}>
            <tile.icon class={tile.off ? 'text-fg-3' : 'text-accent-text'} size={16} aria-hidden="true" />
          </span>
          <div class="flex min-w-0 flex-col">
            <p class="text-sm font-medium text-fg">{tile.title}</p>
            <p class="text-xs text-fg-3">{tile.body}</p>
          </div>
        </li>
      {/each}
    </ul>
  </section>

  <!-- How the app works, in three steps: the loop that happens in the folder chosen above. -->
  <section class="flex flex-col gap-3 [@media(max-height:720px)]:hidden" aria-labelledby="how-it-works">
    <h3 id="how-it-works" class="text-sm font-medium tracking-wider text-fg-3 uppercase">How it works</h3>
    <!-- A timeline: three nodes on one track, left-aligned like the rest of the step. -->
    <!-- Four equal step columns and a narrow repeat column; each step draws its own track to the next ring. -->
    <ol class="grid grid-cols-[repeat(4,minmax(0,1fr))_auto]">
      {#each LOOP as step, index (step.title)}
        <li class="relative flex flex-col items-start gap-2 pr-6">
          <span
            class="absolute top-4 right-2 left-[40px] h-px bg-line-strong"
            aria-hidden="true"
          ></span>
          <!-- The node is SVG, so its ring is always anti-aliased. -->
          <span class="relative grid size-8 place-items-center text-xs font-semibold text-accent-text tabular">
            <svg class="absolute -inset-1 size-10 overflow-visible" viewBox="0 0 40 40" aria-hidden="true">
              <circle cx="20" cy="20" r="15.5" fill="var(--color-accent)" fill-opacity="0.1" stroke="var(--color-accent)" stroke-opacity="0.6" stroke-width="1" />
            </svg>
            <span class="relative">{index + 1}</span>
          </span>
          <p class="mt-1 flex items-center gap-2 text-sm font-medium text-fg"><step.icon class="text-fg-3" size={14} aria-hidden="true" />{step.title}</p>
          <p class="text-xs text-fg-3">{step.body}</p>
          {#if step.keys}<Kbd keys={step.keys} />{/if}
        </li>
      {/each}

      <!-- The loop closes: a dashed repeat ring, built like a step so its title sits level with the others. -->
      <li class="flex flex-col items-start gap-2" aria-label="Then repeat: ask again">
        <span class="relative grid size-8 place-items-center text-accent-text" aria-hidden="true">
          <svg class="absolute -inset-1 size-10" viewBox="0 0 40 40">
            <circle cx="20" cy="20" r="15.5" fill="var(--color-accent)" fill-opacity="0.1" stroke="var(--color-accent)" stroke-opacity="0.6" stroke-width="1" stroke-dasharray="3 3" />
          </svg>
          <Repeat class="relative" size={14} />
        </span>
        <p class="mt-1 text-sm font-medium text-fg">Repeat</p>
        <p class="text-xs text-fg-3">Ask again.</p>
      </li>
    </ol>
  </section>

</div>

<script lang="ts">
  import ArrowLeftRight from '@lucide/svelte/icons/arrow-left-right';
  import FileCog from '@lucide/svelte/icons/file-cog';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import History from '@lucide/svelte/icons/history';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import Button from '$lib/components/Button.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';

  const current = $derived(project.active);
  /** Without a project yet, the remembered ones can be reopened in one click. */
  const reopen = $derived(project.recents.filter((recent) => recent.exists).slice(0, 4));

  /** What the open project turns on; Changes needs a git repository. */
  const tiles = $derived<{ icon: IconComponent; title: string; body: string; off?: boolean }[]>([
    { icon: MessageSquare, title: 'Chat', body: 'Works in this folder.' },
    {
      icon: GitCompare,
      title: 'Changes',
      body: current && !current.isGitRepo ? 'Needs git.' : 'The agent’s diff.',
      off: Boolean(current && !current.isGitRepo),
    },
    { icon: History, title: 'Sessions', body: 'Earlier chats here.' },
    { icon: FileCog, title: 'Project settings', body: 'Beside your global ones.' },
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
        ? 'edge-accent bg-[linear-gradient(135deg,oklch(0.24_0.06_285/0.9),oklch(0.2_0.03_265/0.9))] shadow-[0_18px_50px_-20px_oklch(0.5_0.2_285/0.6)]'
        : 'border-dashed border-line-strong',
    )}
    aria-label={current ? 'Selected project' : 'No project yet'}
  >
    {#if current}
      {@const Icon = current.isGitRepo ? FolderGit2 : Folder}
      <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-[18px]">
        <span class="grid size-[64px] place-items-center rounded-[16px] bg-[linear-gradient(135deg,var(--accent-1),var(--accent-2))] text-white">
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

    <ul class="mt-[22px] grid grid-cols-4 gap-4 border-t border-line pt-[18px]" aria-label="In this project">
      {#each tiles as tile (tile.title)}
        <li class={cx('flex items-start gap-3', (tile.off || !current) && 'opacity-55')}>
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

  <ul class="grid grid-cols-2 gap-6 text-sm text-fg-2 [@media(max-height:720px)]:hidden" aria-label="Good to know">
    <li class="flex items-center gap-3">
      <ShieldCheck class="shrink-0 text-fg-3" size={16} aria-hidden="true" />
      The agent reads and edits files in this folder, as in your terminal.
    </li>
    <li class="flex flex-wrap items-center gap-1.5">
      <ArrowLeftRight class="mr-1.5 shrink-0 text-fg-3" size={16} aria-hidden="true" />
      Switch any time from the top bar or <Kbd keys="mod+k" />
    </li>
  </ul>
</div>

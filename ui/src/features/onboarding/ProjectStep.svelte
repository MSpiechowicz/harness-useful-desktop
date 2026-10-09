<script lang="ts">
  import ArrowLeftRight from '@lucide/svelte/icons/arrow-left-right';
  import FileCog from '@lucide/svelte/icons/file-cog';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import Folders from '@lucide/svelte/icons/folders';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import History from '@lucide/svelte/icons/history';
  import Lightbulb from '@lucide/svelte/icons/lightbulb';
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
  const others = $derived(project.recents.filter((recent) => recent.path !== current?.path && recent.exists));

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

  const LABEL = 'text-sm font-medium tracking-wider text-fg-3 uppercase';
</script>

<div class={cx('mx-auto flex w-full flex-col gap-5 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow="Your project"
    title="Pick a project to work in"
    body="The agent works inside one folder. Optional: you can open or change it any time from the top bar."
  />

  <!-- The project, front and centre: one confident card with the way to change it. -->
  {#if current}
    {@const Icon = current.isGitRepo ? FolderGit2 : Folder}
    <section
      class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-[18px] rounded-[16px] border edge-accent bg-[linear-gradient(135deg,oklch(0.24_0.06_285/0.9),oklch(0.2_0.03_265/0.9))] p-[22px] shadow-[0_0_0_4px_var(--accent-wash),0_18px_50px_-20px_oklch(0.5_0.2_285/0.6)]"
      aria-label="Selected project"
    >
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
    </section>
  {:else}
    <button
      type="button"
      class="group grid grid-cols-[auto_minmax(0,1fr)] items-center gap-5 rounded-xl border border-dashed border-line-strong p-5 text-left transition-colors duration-[var(--dur-base)] hover:border-accent/60 hover:bg-accent-wash"
      onclick={() => void project.pick()}
    >
      <span class="grid size-14 place-items-center rounded-xl bg-accent-wash text-accent-text transition-transform duration-[var(--dur-base)] group-hover:scale-105">
        <FolderOpen size={28} aria-hidden="true" />
      </span>
      <span class="flex flex-col gap-1">
        <span class="text-xl font-semibold tracking-tight text-fg">Open a folder…</span>
        <span class="text-sm text-fg-3">Any folder works; a git repository also unlocks Changes.</span>
      </span>
    </button>
  {/if}

  <section class="flex flex-col gap-3" aria-labelledby="project-tiles">
    <h3 id="project-tiles" class={LABEL}>In this project</h3>
    <ul class="grid grid-cols-4 gap-3">
      {#each tiles as tile (tile.title)}
        <li class={cx('flex items-start gap-3 rounded-lg border border-line bg-surface-1 p-3', tile.off && 'opacity-55')}>
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

  <!-- Two matching panels: where else you can go, and what to keep in mind. -->
  <div class="grid grid-cols-2 gap-4 [@media(max-height:720px)]:hidden">
    <section class="flex flex-col gap-3 rounded-lg border border-line bg-glass p-4" aria-labelledby="other-projects">
      <h3 id="other-projects" class={cx(LABEL, 'flex items-center gap-2')}><Folders size={14} aria-hidden="true" />Other recent projects</h3>
      {#if others.length > 0}
        <ul class="flex flex-wrap gap-2">
          {#each others.slice(0, 6) as recent (recent.path)}
            <li>
              <button
                type="button"
                class="inline-flex h-8 max-w-56 items-center gap-2 rounded-full border border-line-strong bg-surface-1 px-3 text-sm text-fg-2 transition-colors hover:border-accent/50 hover:text-fg"
                title={recent.path}
                onclick={() => void project.open(recent.path)}
              >
                <Folder size={14} aria-hidden="true" /><span class="truncate">{recent.name}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-md border border-dashed border-line-strong text-fg-3">
            <Folders size={18} aria-hidden="true" />
          </span>
          <p class="min-w-0 flex-1 text-sm text-fg-3">Folders you open are remembered here, so switching is one click.</p>
          {#if current}
            <Button variant="ghost" size="sm" onclick={() => void project.pick()}>Open another…</Button>
          {/if}
        </div>
      {/if}
    </section>

    <section class="flex flex-col gap-3 rounded-lg border border-line bg-glass p-4" aria-labelledby="project-notes">
      <h3 id="project-notes" class={cx(LABEL, 'flex items-center gap-2')}><Lightbulb size={14} aria-hidden="true" />Good to know</h3>
      <ul class="flex flex-col gap-3 text-sm text-fg-2">
        <li class="flex items-start gap-3">
          <span class="grid size-7 shrink-0 place-items-center rounded-md bg-surface-2"><ShieldCheck class="text-fg-2" size={15} aria-hidden="true" /></span>
          <span class="pt-1">The agent reads and edits files here, as in your terminal.</span>
        </li>
        <li class="flex items-start gap-3">
          <span class="grid size-7 shrink-0 place-items-center rounded-md bg-surface-2"><ArrowLeftRight class="text-fg-2" size={15} aria-hidden="true" /></span>
          <span class="flex flex-wrap items-center gap-1.5 pt-1">Switch any time from the top bar or <Kbd keys="mod+k" /></span>
        </li>
      </ul>
    </section>
  </div>
</div>

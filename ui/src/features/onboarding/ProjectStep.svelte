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

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import { onboarding } from './onboarding-state.svelte';
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
      return t('onboarding.project.changes.needsGit');
    }

    if (changed === null) {
      return t('onboarding.project.changes.diff');
    }

    if (changed === 0) {
      return branch ? t('onboarding.project.changes.cleanOn', { branch }) : t('onboarding.project.changes.clean');
    }

    return branch ? tn('onboarding.project.changes.countOn', changed, { branch }) : tn('onboarding.project.changes.count', changed);
  }

  /** The app's core loop, taught while the folder it will run in is chosen. */
  const LOOP = $derived<{ icon: IconComponent; title: string; body: string; keys?: string }[]>([
    { icon: MessageSquare, title: t('onboarding.project.loop.ask.title'), body: t('onboarding.project.loop.ask.body'), keys: 'mod+n' },
    { icon: Bot, title: t('onboarding.project.loop.work.title'), body: t('onboarding.project.loop.work.body') },
    { icon: GitCompare, title: t('onboarding.project.loop.review.title'), body: t('onboarding.project.loop.review.body'), keys: 'mod+3' },
    { icon: History, title: t('onboarding.project.loop.resume.title'), body: t('onboarding.project.loop.resume.body'), keys: 'mod+2' },
  ]);

  /** What the open project turns on, with live facts where the app has them. */
  const tiles = $derived<{ icon: IconComponent; title: string; body: string; off?: boolean }[]>([
    {
      icon: MessageSquare,
      title: t('onboarding.project.tile.chat'),
      body: driverName ? t('onboarding.project.tile.chat.ready', { name: driverName }) : t('onboarding.project.tile.chat.idle'),
    },
    { icon: GitCompare, title: t('onboarding.project.tile.changes'), body: changesLine(), off: Boolean(current && !current.isGitRepo) },
    { icon: History, title: t('onboarding.project.tile.sessions'), body: t('onboarding.project.tile.sessions.body') },
    { icon: FileCog, title: t('onboarding.project.tile.settings'), body: t('onboarding.project.tile.settings.body') },
  ]);
</script>

<div class={cx('mx-auto flex w-full flex-col gap-5 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow={t('onboarding.project.eyebrow')}
    title={t('onboarding.project.title')}
    body={t('onboarding.project.body')}
  />

  <!--
    One card is the whole choice: the project (or the way to open one) on top, and what it turns on along the bottom.
    A single crisp border with a soft shadow below; no second ring beside it.
  -->
  <section
    class={cx(
      'flex flex-col rounded-[16px] border-card p-[22px]',
      current
        ? 'edge-accent bg-surface-1 shadow-float'
        : 'border-dashed border-line-strong',
    )}
    aria-label={current ? t('onboarding.project.selectedAria') : t('onboarding.project.noneAria')}
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
              <Badge size="lg"><GitBranch size={14} aria-hidden="true" />{t('onboarding.project.gitRepo')}</Badge>
            {:else}
              <Badge size="lg">{t('onboarding.project.noGit')}</Badge>
            {/if}
            <Badge size="lg" tone="accent">{t('onboarding.project.selected')}</Badge>
          </div>
        </div>
        <!-- Level with the title line (the 36px button centred on its 27px line), not with the middle of the block. -->
        <Button class="-mt-[5px] self-start" onclick={() => void project.pick()}>{t('onboarding.project.change')}</Button>
      </div>
    {:else}
      <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-[18px]">
        <span class="grid size-[64px] place-items-center rounded-[16px] bg-accent-wash text-accent-text">
          <FolderOpen size={30} aria-hidden="true" />
        </span>
        <div class="flex min-w-0 flex-col gap-1">
          <p class="text-[22px] leading-tight font-semibold tracking-tight text-fg">{t('onboarding.project.none')}</p>
          <p class="text-sm text-fg-3">{t('onboarding.project.noneHint')}</p>
          {#if reopen.length > 0}
            <div class="mt-1 flex flex-wrap items-center gap-2 text-xs text-fg-3">
              <span>{t('onboarding.project.reopen')}</span>
              {#each reopen as recent (recent.path)}
                <button
                  type="button"
                  class="inline-flex h-7 max-w-48 items-center gap-2 rounded-full border-card border-line-strong bg-surface-1 px-3 text-sm text-fg-2 transition-colors hover:border-accent/50 hover:text-fg"
                  title={recent.path}
                  onclick={() => void project.open(recent.path)}
                >
                  <Folder size={12} aria-hidden="true" /><span class="truncate">{recent.name}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
        <Button class="-mt-[5px] self-start" variant="primary" onclick={() => void project.pick()}>{t('onboarding.project.openFolder')}</Button>
      </div>
    {/if}

    <!-- Four equal cells with thin dividers, so the spacing reads the same whatever the text length. -->
    <ul class="mt-[22px] grid grid-cols-4 divide-x divide-line border-t border-line pt-[18px]" aria-label={t('onboarding.project.tiles')}>
      {#each tiles as tile (tile.title)}
        <li class={cx('flex items-center gap-3 px-4 first:pl-0 last:pr-0', (tile.off || !current) && 'opacity-55')}>
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
  <section class="mt-4 flex flex-col gap-3 [@media(max-height:720px)]:hidden" aria-labelledby="how-it-works">
    <h3 id="how-it-works" class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.project.how')}</h3>
    <!-- A timeline: three nodes on one track, left-aligned like the rest of the step. -->
    <!-- Four equal step columns and a narrow repeat column; each step draws its own track to the next ring. -->
    <ol class="grid grid-cols-[repeat(4,minmax(0,1fr))_auto]">
      {#each LOOP as step, index (step.title)}
        <li class="relative flex flex-col items-start gap-2 pr-6">
          <span class="absolute top-[15px] right-2 left-[44px] h-0.5 rounded-full bg-accent/45" aria-hidden="true"></span>
          <!-- Solid accent nodes on a visible line, like the Share setup steps. -->
          <span class="relative grid size-8 place-items-center rounded-full bg-accent-fill text-xs font-semibold text-on-accent tabular">{index + 1}</span>
          <p class="mt-1 flex items-center gap-2 text-sm font-medium text-fg"><step.icon class="text-fg-3" size={14} aria-hidden="true" />{step.title}</p>
          <p class="text-xs text-fg-3">{step.body}</p>
          {#if step.keys}<Kbd keys={step.keys} />{/if}
        </li>
      {/each}

      <!-- The loop closes: a dashed repeat ring, built like a step so its title sits level with the others. -->
      <li class="flex flex-col items-start gap-2" aria-label={t('onboarding.project.loop.repeatAria')}>
        <span class="relative grid size-8 place-items-center text-accent-text" aria-hidden="true">
          <!-- The dashes divide the circle exactly (circumference 94.25 = 20 x 4.712), so every gap is the same. -->
          <svg class="absolute inset-0 size-8" viewBox="0 0 32 32">
            <circle cx="16" cy="16" r="15" fill="var(--color-accent)" fill-opacity="0.12" stroke="var(--color-accent)" stroke-width="2" stroke-dasharray="4.712 4.712" />
          </svg>
          <Repeat class="relative" size={14} />
        </span>
        <p class="mt-1 text-sm font-medium text-fg">{t('onboarding.project.loop.repeat.title')}</p>
        <p class="text-xs text-fg-3">{t('onboarding.project.loop.repeat.body')}</p>
      </li>
    </ol>
  </section>

</div>

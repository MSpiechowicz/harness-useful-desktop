<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import FolderX from '@lucide/svelte/icons/folder-x';
  import Settings2 from '@lucide/svelte/icons/settings-2';
  import X from '@lucide/svelte/icons/x';

  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { ICON } from '$lib/components/icon';
  import Popover from '$lib/components/Popover.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { onboarding } from '../onboarding/onboarding.svelte';

  interface Props {
    /** Icon-only trigger for the collapsed rail; the names move into a tooltip. */
    compact?: boolean;
  }

  let { compact = false }: Props = $props();

  let open = $state(false);

  const active = $derived(harness.active);
  const harnessLine = $derived(active ? `${active.info.shortName}${active.detection.version ? ` v${active.detection.version}` : ''}` : 'No harness');
  const projectName = $derived(project.active ? project.active.name : 'No project');
  const summary = $derived(
    `${active ? `Harness: ${active.info.name}` : 'No harness detected'}. ${project.active ? `Project: ${project.active.name}` : 'No project open'}`,
  );

  const ROW = 'flex h-9 w-full items-center gap-2.5 rounded-md px-2 text-left text-sm transition-colors duration-[var(--dur-fast)]';
  const ENABLED = 'text-fg hover:bg-hover active:bg-press';
  const DISABLED = 'cursor-not-allowed text-fg-3';
  const ACTION = 'text-fg-2 hover:bg-hover hover:text-fg active:bg-press';

  async function choose(id: Parameters<typeof harness.select>[0], installed: boolean, close: () => void): Promise<void> {
    if (!installed) {
      return;
    }

    close();
    await harness.select(id);
  }
</script>

<!-- One control for where you are: the harness you talk to and the project it works in. The menu is one flat list:
     harnesses, then projects, then the rare actions. -->
<Popover bind:open label="Harness and project" kind="menu" class="w-64" rootClass={compact ? undefined : 'w-full'}>
  {#snippet trigger({ props })}
    <Tooltip text="{harnessLine} · {projectName}" side="right" disabled={!compact} class={compact ? 'inline-flex' : 'flex w-full'}>
      {#snippet children({ describedby })}
        <button
          type="button"
          {...props}
          aria-label="{summary}. Switch harness or project"
          aria-describedby={compact ? describedby : undefined}
          class={cx(
            'inline-flex items-center rounded-md text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press',
            compact ? 'size-10 justify-center' : 'h-11 w-full gap-2.5 px-1.5',
          )}
        >
          {#if active}
            <HarnessBadge info={active.info} size="sm" />
          {:else}
            <span class="grid size-6 shrink-0 place-items-center"><StatusDot tone="neutral" /></span>
          {/if}

          {#if !compact}
            <span class="flex min-w-0 flex-1 flex-col leading-tight">
              <span class={cx('truncate text-sm font-medium', project.active ? 'text-fg' : 'text-fg-2')}>{projectName}</span>
              <span class="tabular truncate text-xs text-fg-3">{harnessLine}</span>
            </span>
            <ChevronsUpDown size={14} class="shrink-0 text-fg-3" aria-hidden="true" />
          {/if}
        </button>
      {/snippet}
    </Tooltip>
  {/snippet}

  {#snippet children({ close })}
    <div class="flex flex-col gap-0.5" role="group" aria-label="Harness">
      {#each harness.summaries as item (item.info.id)}
        {@const installed = item.detection.installed}
        {@const current = active?.info.id === item.info.id}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={current}
          aria-disabled={!installed}
          aria-label="{item.info.name}{installed ? (item.detection.version ? ` v${item.detection.version}` : '') : '. Not installed'}"
          data-menu-item
          onclick={() => void choose(item.info.id, installed, close)}
          class={cx(ROW, installed ? ENABLED : DISABLED)}
        >
          <HarnessBadge info={item.info} size="sm" class={installed ? '' : 'opacity-60'} />
          <span class="min-w-0 flex-1 truncate">{item.info.name}</span>
          {#if !installed}
            <span class="text-xs text-fg-3">Not installed</span>
          {:else if item.detection.version}
            <span class="tabular text-xs text-fg-3">v{item.detection.version}</span>
          {/if}
          {#if current}<Check size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />{/if}
        </button>
      {/each}
    </div>

    <div class="my-1.5 border-t border-line" role="separator"></div>

    <div class="flex max-h-56 flex-col gap-0.5 overflow-y-auto" role="group" aria-label="Project">
      {#if project.recents.length === 0 && !project.active}
        <p class="px-2 py-1.5 text-sm text-fg-3">No project open</p>
      {/if}

      {#each project.recents as recent (recent.path)}
        {@const current = project.active?.path === recent.path}
        {@const Icon = !recent.exists ? FolderX : recent.isGitRepo ? FolderGit2 : Folder}
        <div class="group/row flex items-center">
          <button
            type="button"
            role="menuitemradio"
            aria-checked={current}
            aria-disabled={!recent.exists}
            aria-label="{recent.name}{recent.exists ? '' : '. Folder is missing'}"
            title={recent.path}
            data-menu-item
            onclick={() => recent.exists && void project.open(recent.path).then(close)}
            class={cx(ROW, 'min-w-0 flex-1', recent.exists ? ENABLED : DISABLED)}
          >
            <Icon size={ICON.sm} class={cx('shrink-0', current ? 'text-accent-text' : 'text-fg-3')} aria-hidden="true" />
            <span class="min-w-0 flex-1 truncate">{recent.name}</span>
            {#if current}<Check size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />{/if}
          </button>
          <!-- Forget shows on hover or focus only; the list stays quiet otherwise. -->
          <button
            type="button"
            aria-label="Forget {recent.name}"
            data-menu-item
            onclick={() => void project.forget(recent.path)}
            class="grid size-7 shrink-0 place-items-center rounded-md text-fg-3 opacity-0 transition-opacity duration-[var(--dur-fast)] group-hover/row:opacity-100 hover:bg-hover hover:text-fg focus-visible:opacity-100"
          >
            <X size={ICON.sm} aria-hidden="true" />
          </button>
        </div>
      {/each}
    </div>

    <div class="my-1.5 border-t border-line" role="separator"></div>

    <div class="flex flex-col gap-0.5">
      <button type="button" role="menuitem" data-menu-item onclick={() => void project.pick().then(close)} class={cx(ROW, ACTION)}>
        <FolderOpen size={ICON.sm} aria-hidden="true" />
        Open folder…
      </button>
      {#if project.active}
        <button type="button" role="menuitem" data-menu-item onclick={() => void project.close().then(close)} class={cx(ROW, ACTION)}>
          <FolderX size={ICON.sm} aria-hidden="true" />
          Close project
        </button>
      {/if}
      <button
        type="button"
        role="menuitem"
        data-menu-item
        onclick={() => {
          close();
          onboarding.restart('harnesses');
        }}
        class={cx(ROW, ACTION)}
      >
        <Settings2 size={ICON.sm} aria-hidden="true" />
        Set up harnesses…
      </button>
    </div>
  {/snippet}
</Popover>

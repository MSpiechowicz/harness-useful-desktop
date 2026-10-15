<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderGit2 from '@lucide/svelte/icons/folder-git-2';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import Popover from '$lib/components/Popover.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import Button from '$lib/components/Button.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';
  import { fuzzyMatch } from '$lib/utils/fuzzy';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import { switchProject, switchToPickedProject } from '../shell/actions';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let open = $state(false);
  let query = $state('');
  /** Edit mode: rows become checkboxes and the ticked ones leave the list (the folders stay on disk). */
  let editing = $state(false);
  let ticked = $state<string[]>([]);
  let removing = $state(false);

  const path = $derived(session.info.project);
  const name = $derived(path.split(/[\\/]/).filter(Boolean).at(-1) ?? path);
  const others = $derived(project.recents.filter((recent) => recent.exists));
  const shown = $derived(others.filter((recent) => fuzzyMatch(query, `${recent.name} ${recent.path}`) !== null));

  // A closed menu starts fresh next time.
  $effect(() => {
    if (!open) {
      query = '';
      editing = false;
      ticked = [];
    }
  });

  function tick(target: string): void {
    ticked = ticked.includes(target) ? ticked.filter((entry) => entry !== target) : [...ticked, target];
  }

  async function removeTicked(): Promise<void> {
    removing = true;
    for (const target of ticked) {
      await project.forget(target);
    }
    removing = false;
    ticked = [];
    editing = false;
  }
  /** A chat that moves with its project (OMP) cannot move while the agent works. */
  const locked = $derived(session.streaming && Boolean(harness.get(session.info.harness)?.capabilities.moveSession));

  const ROW = 'flex w-full items-center gap-2.5 rounded-sm px-2.5 py-1.5 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press focus-visible:bg-hover focus-visible:-outline-offset-2';

  function choose(target: string, close: () => void): void {
    close();
    if (target !== path) {
      void switchProject(target);
    }
  }
</script>

<!-- The chat's project, and a quick way to work in another one: its open chat, or a new chat there. -->
<Popover label={t('chat.project.popover')} kind="dialog" side="top" align="start" bind:open class="w-max max-w-md!" rootClass="max-w-[40%] shrink">
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      title={locked ? t('chat.project.locked') : path}
      disabled={locked}
      aria-label={t('chat.project.aria', { name })}
      class="inline-flex h-control max-w-full min-w-0 items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-fg-3 aria-expanded:bg-hover aria-expanded:text-fg"
    >
      <Folder size={14} aria-hidden="true" class="shrink-0" />
      <span class="-mb-[3px] min-w-0 truncate pb-[3px] [text-box:trim-both_cap_alphabetic] @max-[460px]:hidden">{name}</span>
      <ChevronDown size={16} aria-hidden="true" class="shrink-0 text-fg-3 @max-[460px]:hidden" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <!-- Home/End belong to the search field, not to the list navigation of the popover. -->
    <div class="flex w-80 max-w-full flex-col gap-1" role="presentation" onkeydown={(event) => (event.key === 'Home' || event.key === 'End') && event.stopPropagation()}>
      <SearchField bind:value={query} label={t('chat.project.search')} placeholder={t('chat.project.search')} autofocus />

      <div class="flex items-center justify-between px-2.5 pt-1.5">
        <span class="text-xs font-medium tracking-wide text-fg-3 uppercase">{t('chat.project.recent')}</span>
        <button type="button" data-menu-item class="rounded-sm px-1.5 text-xs text-accent-text hover:bg-hover" aria-pressed={editing} onclick={() => ((editing = !editing), (ticked = []))}>
          {editing ? t('chat.project.done') : t('chat.project.edit')}
        </button>
      </div>

      <div class="flex max-h-72 flex-col gap-0.5 overflow-y-auto" role="group" aria-label={t('chat.project.recent')}>
        {#if shown.length === 0}
          <p class="px-2.5 py-3 text-sm text-fg-3" role="status">{t('chat.project.none')}</p>
        {/if}

        {#each shown as recent (recent.path)}
          {@const current = recent.path === path}
          {@const Icon = recent.isGitRepo ? FolderGit2 : Folder}
          {@const on = ticked.includes(recent.path)}
          <button
            type="button"
            role={editing ? 'checkbox' : 'menuitemradio'}
            aria-checked={editing ? on : current}
            disabled={editing && current}
            title={editing && current ? t('chat.project.keep') : recent.path}
            data-menu-item
            class={cx(ROW, current ? 'text-fg' : 'text-fg-2', 'disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-transparent')}
            onclick={() => (editing ? tick(recent.path) : choose(recent.path, close))}
          >
            {#if editing}
              <span class={cx('grid size-4 shrink-0 place-items-center rounded-[5px] border transition-colors duration-[var(--dur-fast)]', on ? 'border-accent-fill bg-accent-fill text-on-accent' : 'border-line-strong')} aria-hidden="true">
                {#if on}<Check size={12} strokeWidth={3} />{/if}
              </span>
            {/if}
            <Icon size={16} aria-hidden="true" class={cx('shrink-0', current ? 'text-accent-text' : 'text-fg-3')} />
            <span class="min-w-0 flex-1 truncate">{recent.name}</span>
            {#if !editing}
              <Check size={16} aria-hidden="true" class={cx('shrink-0 text-accent-text', !current && 'invisible')} />
            {/if}
          </button>
        {/each}
      </div>

      <div class="my-1 border-t border-line" role="separator"></div>

      {#if editing}
        <Button variant="dangerQuiet" disabled={ticked.length === 0 || removing} loading={removing} onclick={() => void removeTicked()}>
          <Trash2 size={16} aria-hidden="true" />
          {tn('chat.project.remove', Math.max(ticked.length, 1))}
        </Button>
      {:else}
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
          {t('chat.project.open')}
        </button>
      {/if}
    </div>
  {/snippet}
</Popover>

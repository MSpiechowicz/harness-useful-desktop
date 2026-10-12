<script lang="ts">
  import CornerDownLeft from '@lucide/svelte/icons/corner-down-left';
  import SearchX from '@lucide/svelte/icons/search-x';
  import Search from '@lucide/svelte/icons/search';

  import { t } from '$lib/i18n/i18n.svelte';
  import { paletteRecents } from '$lib/stores/palette-recents.svelte';
  import { uid } from '$lib/utils/ids';

  import HighlightedText from './HighlightedText.svelte';
  import { ICON } from './icon';
  import Kbd from './Kbd.svelte';
  import Overlay from './Overlay.svelte';
  import { MODE_PREFIXES, recentGroup, searchPalette, type PaletteCommand } from './palette';

  interface Props {
    open?: boolean;
    commands: PaletteCommand[];
    placeholder?: string;
  }

  let { open = $bindable(false), commands, placeholder }: Props = $props();

  const listId = uid('palette-list');

  let query = $state('');
  let activeIndex = $state(0);
  let input: HTMLInputElement | undefined = $state();

  const results = $derived(searchPalette(commands, query, { recents: paletteRecents.ids }));
  const groups = $derived(results.groups);
  const flat = $derived(groups.flatMap((group) => group.entries));
  const activeEntry = $derived(flat[activeIndex]);
  const modeInfo = $derived(MODE_PREFIXES.find((entry) => entry.mode === results.mode));

  function optionId(commandId: string): string {
    return `${listId}-${commandId}`;
  }

  // A new query always restarts at the best match (the first entry that can run).
  $effect(() => {
    void query;
    activeIndex = Math.max(0, flat.findIndex((entry) => !entry.command.disabled));
  });

  $effect(() => {
    if (activeEntry) {
      document.getElementById(optionId(activeEntry.command.id))?.scrollIntoView({ block: 'nearest' });
    }
  });

  function close(): void {
    open = false;
    query = '';
  }

  function run(command: PaletteCommand | undefined, secondary = false): void {
    if (!command || command.disabled) {
      return;
    }

    const action = secondary ? command.secondary?.run : command.run;
    if (!action) {
      return;
    }

    // Only stable entries are worth remembering; a session or file may be gone tomorrow.
    if (command.kind === undefined || command.kind === 'nav' || command.kind === 'action') {
      paletteRecents.record(command.id);
    }

    close();
    action();
  }

  function move(delta: number): void {
    if (flat.length === 0) {
      return;
    }

    // Entries that cannot run (the view you are on) are shown but skipped.
    for (let step = 1; step <= flat.length; step += 1) {
      const next = (activeIndex + delta * step + flat.length * step) % flat.length;

      if (!flat[next].command.disabled) {
        activeIndex = next;

        return;
      }
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      move(1);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      move(-1);
    } else if (event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      run(activeEntry?.command, event.shiftKey);
    }
  }

  function usePrefix(prefix: string): void {
    query = prefix;
    input?.focus();
  }

  const emptyMessage = $derived.by(() => {
    if (results.mode !== 'all' && results.text.trim() === '') {
      return t('components.palette.emptyMode', { noun: modeInfo?.noun ?? t('components.palette.noun.results') });
    }

    return t('components.palette.noResults', { query: results.text.trim() });
  });
</script>

<Overlay {open} placement="top" label={t('components.palette.label')} onclose={close} class="w-[40rem]">
  <div class="overflow-hidden rounded-xl border border-line-strong bg-glass shadow-pop backdrop-blur-xl">
    <div class="flex items-center gap-3 border-b border-line px-4">
      <Search class="shrink-0 text-fg-3" size={ICON.lg} aria-hidden="true" />
      <input
        bind:this={input}
        bind:value={query}
        onkeydown={onKeydown}
        type="text"
        role="combobox"
        aria-label={t('components.palette.input')}
        aria-expanded="true"
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={activeEntry ? optionId(activeEntry.command.id) : undefined}
        autocomplete="off"
        spellcheck="false"
        placeholder={placeholder ?? t('components.palette.placeholder')}
        class="h-14 min-w-0 flex-1 bg-transparent text-lg text-fg outline-none placeholder:text-fg-3"
      />
      {#if modeInfo}
        <span class="shrink-0 rounded-full bg-accent-wash px-2.5 py-0.5 text-xs font-medium text-accent-text" aria-live="polite">{modeInfo.label}</span>
      {/if}
      <Kbd keys="escape" />
    </div>

    <div id={listId} role="listbox" aria-label={t('components.palette.results')} class="max-h-[50vh] overflow-y-auto p-2">
      {#each groups as group (group.name)}
        <div role="group" aria-label={group.name}>
          <p class="flex items-baseline gap-2 px-3 pt-2 pb-1 text-xs font-medium tracking-wider text-fg-3 uppercase" aria-hidden="true">
            {group.name}
            {#if group.hidden > 0}
              <span class="font-normal tracking-normal normal-case">{t('components.palette.more', { count: group.hidden })}</span>
            {/if}
          </p>

          {#each group.entries as entry (entry.command.id)}
            {@const command = entry.command}
            {@const selected = entry === activeEntry}
            {@const Icon = command.icon}
            <button
              type="button"
              role="option"
              id={optionId(command.id)}
              tabindex="-1"
              aria-selected={selected}
              aria-disabled={command.disabled || undefined}
              onclick={() => run(command)}
              onpointermove={() => (activeIndex = flat.indexOf(entry))}
              class="flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left text-base transition-colors duration-[var(--dur-fast)] active:bg-press {selected
                ? 'bg-accent-wash text-fg'
                : 'text-fg-2'} {command.disabled ? 'opacity-50' : ''}"
            >
              {#if Icon}
                <Icon class={selected ? 'text-accent-text' : 'text-fg-3'} size={ICON.md} strokeWidth={1.8} aria-hidden="true" />
              {/if}

              <span class="flex min-w-0 flex-1 items-baseline gap-2">
                <span class="truncate">
                  <HighlightedText text={command.label} indices={entry.labelIndices} />
                </span>
                {#if command.detail}
                  <span class="min-w-0 truncate text-sm text-fg-3">{command.detail}</span>
                {/if}
              </span>

              {#if selected && command.secondary}
                <span class="inline-flex shrink-0 items-center gap-1.5 text-xs text-fg-3"><Kbd keys="shift+enter" /> {command.secondary.label}</span>
              {:else if command.shortcut}
                <Kbd keys={command.shortcut} />
              {:else if selected && group.name !== recentGroup()}
                <CornerDownLeft size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
              {/if}
            </button>
          {/each}
        </div>
      {:else}
        <div class="flex flex-col items-center gap-3 px-3 py-8 text-center" role="status">
          <SearchX size={ICON.lg} class="text-fg-3" aria-hidden="true" />
          <p class="text-base text-fg-2">{emptyMessage}</p>

          {#if results.mode === 'all'}
            <p class="text-sm text-fg-3">{t('components.palette.hint')}</p>
            <ul class="flex flex-wrap justify-center gap-2" aria-label={t('components.palette.narrow')}>
              {#each MODE_PREFIXES as entry (entry.prefix)}
                <li>
                  <button type="button" class="chip px-3" onclick={() => usePrefix(entry.prefix)}>
                    <kbd class="font-mono text-accent-text">{entry.prefix}</kbd>
                    {entry.label}
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </div>

    <footer class="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-t border-line px-4 py-2.5 text-xs text-fg-3">
      <span class="inline-flex items-center gap-1.5"><Kbd keys="arrowup" /><Kbd keys="arrowdown" /> {t('components.palette.navigate')}</span>
      <span class="inline-flex items-center gap-1.5"><Kbd keys="enter" /> {t('components.palette.run')}</span>

      <ul class="ml-auto flex items-center gap-3" aria-label={t('components.palette.modes')}>
        {#each MODE_PREFIXES as entry (entry.prefix)}
          <li>
            <button type="button" class="inline-flex items-center gap-1.5 rounded-sm px-1 transition-colors hover:text-fg" onclick={() => usePrefix(entry.prefix)}>
              <kbd class="grid h-5 min-w-5 place-items-center rounded-sm border border-line-strong bg-surface-2 px-1 font-mono text-xs text-fg-2">{entry.prefix}</kbd>
              {entry.label}
            </button>
          </li>
        {/each}
      </ul>
    </footer>
  </div>
</Overlay>

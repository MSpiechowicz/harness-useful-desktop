<script lang="ts">
  import Search from '@lucide/svelte/icons/search';

  import { uid } from '$lib/utils/ids';

  import HighlightedText from './HighlightedText.svelte';
  import Kbd from './Kbd.svelte';
  import Overlay from './Overlay.svelte';
  import { searchCommands, type PaletteCommand } from './palette';

  interface Props {
    open?: boolean;
    commands: PaletteCommand[];
    placeholder?: string;
  }

  let { open = $bindable(false), commands, placeholder = 'Type a command or search…' }: Props = $props();

  const listId = uid('palette-list');

  let query = $state('');
  let activeIndex = $state(0);

  const groups = $derived(searchCommands(commands, query));
  const flat = $derived(groups.flatMap((group) => group.entries));
  const activeEntry = $derived(flat[activeIndex]);

  function optionId(commandId: string): string {
    return `${listId}-${commandId}`;
  }

  // A new query always restarts at the best match.
  $effect(() => {
    void query;
    activeIndex = 0;
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

  function run(command: PaletteCommand | undefined): void {
    if (!command || command.disabled) {
      return;
    }

    close();
    command.run();
  }

  function move(delta: number): void {
    if (flat.length === 0) {
      return;
    }

    activeIndex = (activeIndex + delta + flat.length) % flat.length;
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      move(1);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      move(-1);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      run(activeEntry?.command);
    }
  }
</script>

<Overlay {open} placement="top" label="Command palette" onclose={close} class="w-[40rem]">
  <div class="overflow-hidden rounded-xl border border-line-strong bg-glass shadow-pop backdrop-blur-xl">
    <div class="flex items-center gap-3 border-b border-line px-4">
      <Search class="shrink-0 text-fg-3" size={18} aria-hidden="true" />
      <input
        bind:value={query}
        onkeydown={onKeydown}
        type="text"
        role="combobox"
        aria-label="Search commands"
        aria-expanded="true"
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={activeEntry ? optionId(activeEntry.command.id) : undefined}
        autocomplete="off"
        spellcheck="false"
        {placeholder}
        class="h-14 flex-1 bg-transparent text-lg text-fg outline-none placeholder:text-fg-3"
      />
      <Kbd keys="escape" />
    </div>

    <div id={listId} role="listbox" aria-label="Commands" class="max-h-[50vh] overflow-y-auto p-2">
      {#each groups as group (group.name)}
        <div role="group" aria-label={group.name}>
          <p class="px-3 pt-2 pb-1 text-xs font-medium tracking-wider text-fg-3 uppercase" aria-hidden="true">{group.name}</p>

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
              class="flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left text-base transition-colors duration-[var(--dur-fast)] {selected
                ? 'bg-accent-wash text-fg'
                : 'text-fg-2'} {command.disabled ? 'opacity-50' : ''}"
            >
              {#if Icon}
                <Icon class={selected ? 'text-accent-text' : 'text-fg-3'} size={18} strokeWidth={1.8} aria-hidden="true" />
              {/if}

              <span class="flex-1 truncate">
                <HighlightedText text={command.label} indices={entry.labelIndices} />
              </span>

              {#if command.shortcut}
                <Kbd keys={command.shortcut} />
              {/if}
            </button>
          {/each}
        </div>
      {:else}
        <p class="px-3 py-8 text-center text-base text-fg-3" role="status">No matching commands</p>
      {/each}
    </div>

    <footer class="flex items-center gap-4 border-t border-line px-4 py-2.5 text-xs text-fg-3">
      <span class="inline-flex items-center gap-1.5"><Kbd keys="arrowup" /><Kbd keys="arrowdown" /> navigate</span>
      <span class="inline-flex items-center gap-1.5"><Kbd keys="enter" /> run</span>
    </footer>
  </div>
</Overlay>

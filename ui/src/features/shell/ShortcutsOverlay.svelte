<script lang="ts">
  import Keyboard from '@lucide/svelte/icons/keyboard';
  import X from '@lucide/svelte/icons/x';

  import IconButton from '$lib/components/IconButton.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import Overlay from '$lib/components/Overlay.svelte';
  import { uid } from '$lib/utils/ids';

  import type { ShortcutGroup } from './shortcuts';

  interface Props {
    open: boolean;
    groups: ShortcutGroup[];
    onclose: () => void;
  }

  let { open, groups, onclose }: Props = $props();

  const titleId = uid('shortcuts-title');
</script>

<Overlay {open} labelledby={titleId} {onclose} class="w-[56rem]">
  <div class="flex max-h-[85vh] flex-col overflow-hidden rounded-xl border border-line-strong bg-glass shadow-pop backdrop-blur-xl">
    <header class="flex items-center gap-3 border-b border-line px-6 py-4">
      <span class="grid size-9 place-items-center rounded-md border border-line bg-surface-2 text-accent-text shadow-raised">
        <Keyboard size={ICON.md} aria-hidden="true" />
      </span>

      <div class="flex min-w-0 flex-1 flex-col">
        <h2 id={titleId} class="text-lg text-fg">Keyboard shortcuts</h2>
        <p class="text-sm text-fg-2">Everything has a key. Press <Kbd keys="shift+?" /> anywhere to see this again.</p>
      </div>

      <IconButton icon={X} label="Close shortcuts" size="sm" side="bottom" data-dismiss onclick={onclose} />
    </header>

    <div class="grid grid-cols-1 gap-x-10 gap-y-7 overflow-y-auto px-6 py-6 md:grid-cols-2" data-testid="shortcut-groups">
      {#each groups as group (group.area)}
        <section aria-labelledby="{titleId}-{group.area}" class="flex flex-col gap-1">
          <h3 id="{titleId}-{group.area}" class="pb-1 text-xs font-medium tracking-wider text-fg-3 uppercase">{group.area}</h3>

          <ul class="flex flex-col">
            {#each group.rows as row, index (`${index}:${row.label}`)}
              <li class="flex items-center justify-between gap-4 rounded-sm px-2 py-1.5 transition-colors hover:bg-hover">
                <span class="text-base text-fg-2">{row.label}</span>

                <span class="flex shrink-0 items-center gap-1.5">
                  {#each row.combos as combo, index (combo)}
                    {#if index > 0 && row.joiner === 'or'}<span class="text-xs text-fg-3">or</span>{/if}
                    <Kbd keys={combo} />
                  {/each}
                </span>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>
  </div>
</Overlay>

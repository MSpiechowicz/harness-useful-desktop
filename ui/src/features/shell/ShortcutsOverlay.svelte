<script lang="ts">
  import Keyboard from '@lucide/svelte/icons/keyboard';
  import X from '@lucide/svelte/icons/x';

  import IconButton from '$lib/components/IconButton.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import Overlay from '$lib/components/Overlay.svelte';
  import { uid } from '$lib/utils/ids';

  import { areaLabel, type ShortcutGroup } from './shortcuts';

  interface Props {
    open: boolean;
    groups: ShortcutGroup[];
    onclose: () => void;
  }

  let { open, groups, onclose }: Props = $props();

  const titleId = uid('shortcuts-title');

  // The hint holds a key cap in the middle of a sentence: split around a marker the cap replaces.
  const KEYS_MARK = '\u0000';
  const hint = $derived(t('shell.shortcuts.hint', { keys: KEYS_MARK }).split(KEYS_MARK));
</script>

<Overlay {open} labelledby={titleId} {onclose} class="w-[56rem]">
  <div class="flex max-h-[85vh] flex-col overflow-hidden rounded-xl border border-line-strong bg-glass shadow-pop backdrop-blur-xl">
    <header class="flex items-start gap-3 border-b border-line px-6 py-4">
      <span class="grid size-9 place-items-center rounded-md border border-line bg-surface-2 text-accent-text shadow-raised">
        <Keyboard size={ICON.md} aria-hidden="true" />
      </span>

      <div class="flex min-w-0 flex-1 flex-col">
        <h2 id={titleId} class="text-lg text-fg">{t('shell.shortcuts.title')}</h2>
        <p class="text-sm text-fg-2">{hint[0]}<Kbd keys="shift+?" />{hint[1]}</p>
      </div>

      <IconButton icon={X} label={t('shell.shortcuts.close')} side="bottom" class="-mt-1.5 -mr-1.5 shrink-0" data-dismiss onclick={onclose} />
    </header>

    <div class="grid grid-cols-1 gap-x-10 gap-y-7 overflow-y-auto px-6 pt-6 pb-2.5 md:grid-cols-2" data-testid="shortcut-groups">
      {#each groups as group (group.area)}
        <section aria-labelledby="{titleId}-{group.area}" class="flex flex-col gap-1">
          <h3 id="{titleId}-{group.area}" class="pb-1 text-xs font-medium tracking-wider text-fg-3 uppercase">{areaLabel(group.area)}</h3>

          <ul class="flex flex-col">
            {#each group.rows as row, index (`${index}:${row.label}`)}
              <li class="flex items-center justify-between gap-4 rounded-sm px-2 py-1.5 transition-colors hover:bg-hover">
                <span class="text-base text-fg-2">{row.label}</span>

                <span class="flex shrink-0 items-center gap-1.5">
                  {#each row.combos as combo, index (combo)}
                    {#if index > 0 && row.joiner === 'or'}<span class="text-xs text-fg-3">{t('shell.shortcuts.or')}</span>{/if}
                    <Kbd keys={combo} even />
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

<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import HighlightedText from '$lib/components/HighlightedText.svelte';
  import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
  import { fuzzyMatch } from '$lib/utils/fuzzy';
  import { cx } from '$lib/utils/cx';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    id: string;
    commands: CommandSpec[];
    query: string;
    active: number;
    onaccept: (command: CommandSpec) => void;
    onhover: (index: number) => void;
  }

  let { id, commands, query, active, onaccept, onhover }: Props = $props();
</script>

<!-- Pointer picks only; the textarea keeps focus and drives the list with the keyboard (aria-activedescendant). -->
<ul {id} role="listbox" aria-label={t('chat.slash.label')} class="absolute inset-x-0 bottom-full z-30 mb-2 max-h-72 overflow-y-auto rounded-lg border border-line-strong bg-glass p-1.5 shadow-pop backdrop-blur-xl">
  {#each commands as command, index (command.name)}
    <!-- svelte-ignore a11y_click_events_have_key_events -- the textarea owns the keyboard -->
    <li
      id="{id}-{index}"
      role="option"
      aria-selected={index === active}
      class={cx('flex cursor-pointer items-baseline gap-3 rounded-sm px-2.5 py-1.5 text-sm', index === active ? 'bg-hover' : '')}
      onmousedown={(event) => event.preventDefault()}
      onclick={() => onaccept(command)}
      onpointerenter={() => onhover(index)}
    >
      <span class="shrink-0 font-mono text-fg">/<HighlightedText text={command.name} indices={fuzzyMatch(query, command.name)?.indices ?? []} />{#if command.argumentHint}<span class="ml-1.5 text-fg-3">{command.argumentHint}</span>{/if}</span>
      <span class="min-w-0 flex-1 truncate text-fg-2">{command.description}</span>
      <Badge class="shrink-0">{command.source}</Badge>
    </li>
  {/each}
</ul>

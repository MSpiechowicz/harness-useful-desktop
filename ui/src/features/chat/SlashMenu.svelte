<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import HighlightedText from '$lib/components/HighlightedText.svelte';
  import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
  import { fuzzyMatch } from '$lib/utils/fuzzy';
  import { t } from '$lib/i18n/i18n.svelte';

  import SuggestionMenu from './SuggestionMenu.svelte';

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

<SuggestionMenu {id} label={t('chat.slash.label')} items={commands} {active} rowClass="items-baseline" itemKey={(command) => command.name} {onaccept} {onhover}>
  {#snippet row(command)}
    <span class="shrink-0 font-mono text-fg">/<HighlightedText text={command.name} indices={fuzzyMatch(query, command.name)?.indices ?? []} />{#if command.argumentHint}<span class="ml-1.5 text-fg-3">{command.argumentHint}</span>{/if}</span>
    <span class="min-w-0 flex-1 truncate text-fg-2">{command.description}</span>
    <Badge class="shrink-0">{command.source}</Badge>
  {/snippet}
</SuggestionMenu>

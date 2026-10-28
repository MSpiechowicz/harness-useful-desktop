<script lang="ts">
  import NotebookText from '@lucide/svelte/icons/notebook-text';
  import type { ComponentProps } from 'svelte';

  import { parseNoteTitles, readNotes } from '$lib/chat/memory-tools';
  import { tn } from '$lib/i18n/i18n.svelte';

  import MemoryCallRow from './MemoryCallRow.svelte';
  import ToolResult from './ToolResult.svelte';

  type Props = Pick<ComponentProps<typeof MemoryCallRow>, 'call' | 'active'>;

  let { call, active }: Props = $props();

  const titles = $derived(parseNoteTitles(call.resultText));
  /** The notes the reply shows, else the ones the call asked for: a call that is still going has only the second. */
  const count = $derived(readNotes(call.args, call.resultText).length);
</script>

<MemoryCallRow {call} {active} icon={NotebookText} heading={tn('chat.memory.read.title', Math.max(count, 1))} hint={titles[0] ?? null} done={count > 0 ? { tone: 'info', label: tn('chat.memory.read.count', count) } : null}>
  {#snippet content()}
    <div class="px-4 py-4"><ToolResult text={call.resultText} running={false} failed={false} /></div>
  {/snippet}
</MemoryCallRow>

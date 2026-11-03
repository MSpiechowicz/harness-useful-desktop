<script lang="ts">
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { knowledgeRecallRows, memoryRecallRows } from '$lib/chat/recall-rows';
  import type { StepRecall } from '$lib/chat/step-recall';

  import { knowledgeCounts, memoryCounts } from './recall-counts';

  interface Props {
    /** What the app found for the step called `name`. */
    recall: StepRecall;
    name: string;
  }

  let { recall, name }: Props = $props();

  const panel = 'chat-radius max-h-72 overflow-auto border border-line bg-canvas px-3.5 py-3 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-fg-2';

  // What the app found is a card under the message when its block lists rows the app can read, so the step only says how many.
  const shown = $derived.by(() => {
    if (recall.kind === 'knowledge') {
      const rows = knowledgeRecallRows(recall.body);

      return rows ? knowledgeCounts(rows) : null;
    }

    const rows = memoryRecallRows(recall.body);

    return rows ? memoryCounts(rows) : null;
  });

  /** "2 notes", "1 script" or "2 notes · 1 script": a part with nothing in it is left out. */
  const counts = $derived.by(() => {
    if (recall.kind === 'knowledge') {
      return recall.pointers.length > 0 ? tn('hooks.chip.recall.pointers', recall.pointers.length) : '';
    }

    return [recall.notes > 0 ? tn('chat.user.recallCount', recall.notes) : '', recall.scripts > 0 ? tn('chat.user.recallScriptCount', recall.scripts) : ''].filter(Boolean).join(' · ');
  });
</script>

<!-- What the app found for a step, readable instead of as the tags the agent gets. -->
<div class="flex flex-col gap-1.5">
  <p class="text-xs text-fg-3">
    {t(recall.kind === 'memory' ? 'hooks.chip.recall.memory' : 'hooks.chip.recall.knowledge')}
    {#if shown}
      <span class="ml-1 text-fg-2">· {t('hooks.chip.recall.below', { counts: shown })}</span>
    {:else if counts}
      <span class="ml-1 text-fg-2">· {counts}</span>
    {/if}
  </p>

  {#if shown !== null}
    <!-- The rows are in the card under the message. -->
  {:else if recall.kind === 'knowledge' && recall.pointers.length > 0}
    <ul role="region" aria-label={t('hooks.chip.recall.label', { name })} class="flex flex-col gap-2">
      {#each recall.pointers as pointer, index (index)}
        <li class="chat-radius flex flex-col gap-0.5 border border-line bg-canvas px-3.5 py-2.5">
          <span class="text-sm font-medium text-fg">{pointer.title}{#if pointer.section}<span class="ml-2 text-xs font-normal text-fg-3">{pointer.section}</span>{/if}</span>
          {#if pointer.text}<span class="text-sm break-words text-fg-2">{pointer.text}</span>{/if}
        </li>
      {/each}
    </ul>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
    <pre role="region" tabindex="0" aria-label={t('hooks.chip.recall.label', { name })} class={panel}>{recall.body}</pre>
  {/if}
</div>

<script lang="ts">
  import { nearEnd, panelScroller } from '$lib/chat/subagent-follow';
  import { liveStepId } from '$lib/chat/subagent-steps';
  import type { SubagentStepEntry } from '$lib/chat/types';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { Subagent } from '$lib/ipc/bindings/Subagent';

  import SubagentStepRow from './SubagentStepRow.svelte';

  interface Props {
    subagent: Subagent;
    entries: SubagentStepEntry[];
    startedAt: number | null;
  }

  let { subagent, entries, startedAt }: Props = $props();

  let list: HTMLElement | undefined = $state();
  let stuck = true;

  const live = $derived(subagent.status === 'running' ? liveStepId(entries) : null);

  // New steps keep the end in view while it was in view; a person reading further up is left where they are.
  $effect.pre(() => {
    void entries.length;
    const scroller = panelScroller(list);

    if (scroller) {
      stuck = nearEnd(scroller);
    }
  });

  $effect(() => {
    void entries.length;
    const scroller = panelScroller(list);

    if (scroller && stuck) {
      scroller.scrollTop = scroller.scrollHeight;
    }
  });
</script>

{#if entries.length === 0}
  <p class="py-2 text-sm text-fg-3">{t('subagents.steps.empty')}</p>
{:else}
  <ul bind:this={list} class="m-0 list-none p-0" aria-label={t('subagents.steps')}>
    {#each entries as entry (entry.id)}
      <SubagentStepRow {entry} live={entry.id === live} {startedAt} />
    {/each}
  </ul>
{/if}

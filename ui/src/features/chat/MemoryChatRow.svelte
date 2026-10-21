<script lang="ts">
  import FileCode from '@lucide/svelte/icons/file-code';
  import NotebookText from '@lucide/svelte/icons/notebook-text';
  import Brain from '@lucide/svelte/icons/brain';
  import Pause from '@lucide/svelte/icons/pause';
  import X from '@lucide/svelte/icons/x';

  import { t } from '$lib/i18n/i18n.svelte';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';

  import { chatMemoryState, memoryLine } from './memory-rows';
  import MemoryWaiting from './MemoryWaiting.svelte';

  interface Props {
    /** The key of the chat's agent. */
    agentKey: string;
  }

  let { agentKey }: Props = $props();

  const chatState = $derived(chatMemoryState(agentKey));

  const line = $derived(chatState ? memoryLine(chatState) : []);
</script>

<div class="flex flex-col gap-3">
  {#if chatState && chatState.waiting.length > 0}
    <MemoryWaiting {agentKey} />
  {/if}

  {#each chatState?.outcomes ?? [] as outcome (outcome.id)}
    <p class="flex items-center justify-center gap-2 text-xs text-fg-3" role="status">
      {#if outcome.what === 'skipped'}
        <X size={14} aria-hidden="true" />
        {t('memory.guards.line.skipped', { title: outcome.title })}
      {:else if outcome.kind === 'note'}
        <NotebookText size={14} aria-hidden="true" />
        {t('memory.guards.line.savedNote', { title: outcome.title })}
      {:else}
        <FileCode size={14} aria-hidden="true" />
        {t('memory.guards.line.savedScript', { title: outcome.title })}
      {/if}
    </p>
  {/each}

  {#if chatState?.paused}
    <p class="flex items-center justify-center gap-2 text-xs text-fg-3" role="status">
      <Pause size={14} aria-hidden="true" />
      {t('memory.guards.line.paused')}
      <button type="button" class="rounded-sm text-accent-text transition-colors duration-[var(--dur-fast)] hover:text-fg" onclick={() => void memoryChat.setPaused(agentKey, false)}>{t('memory.guards.resume')}</button>
    </p>
  {:else if line.length > 0}
    <p class="flex items-center justify-center gap-2 text-xs text-fg-3" role="status">
      <Brain size={14} aria-hidden="true" />
      {line.map((sentence) => t(sentence.key, sentence.params)).join(' ')}
    </p>
  {/if}
</div>

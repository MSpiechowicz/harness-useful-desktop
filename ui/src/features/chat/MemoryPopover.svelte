<script lang="ts">
  import Hand from '@lucide/svelte/icons/hand';
  import Brain from '@lucide/svelte/icons/brain';
  import Pause from '@lucide/svelte/icons/pause';
  import Play from '@lucide/svelte/icons/play';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { everythingOff } from '$lib/stores/memory-chat';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';

  import MemoryKindCard from './MemoryKindCard.svelte';

  interface Props {
    agentKey: string;
    root: string | null;
    repoName: string;
    /** How many saves wait for the user in this chat. */
    waiting: number;
    onclose: () => void;
  }

  let { agentKey, root, repoName, waiting, onclose }: Props = $props();

  /** The spinner stays this long at least: sending is quick, and the click should visibly do something. */
  const CHECK_MIN_MS = 700;

  let checking = $state(false);

  const guards = $derived(memoryChat.get(agentKey)?.guards ?? null);
  const nothingToSave = $derived(guards ? everythingOff(guards, memoryChat.guards, root) : true);

  function review(): void {
    memoryChat.requestReview(agentKey);
    onclose();
  }

  async function checkAgain(): Promise<void> {
    checking = true;

    try {
      await Promise.all([memoryChat.checkAgain(agentKey), new Promise((resolve) => setTimeout(resolve, CHECK_MIN_MS))]);
    } finally {
      checking = false;
    }
  }
</script>

{#if guards}
  <div class="flex flex-col">
    <header class="flex items-center gap-2.5 px-4 pt-3 pb-2.5">
      <Brain size={ICON.sm} class="text-fg-2" aria-hidden="true" />
      <h2 class="text-sm font-semibold text-fg">{t('memory.guards.heading')}</h2>
      <span class="ml-auto text-xs text-fg-2">{t('memory.guards.readingStaysOn')}</span>
    </header>

    {#if waiting > 0}
      <div class="flex items-center gap-2.5 border-t border-line bg-warning-wash py-2 pr-2 pl-4 text-sm text-warning" role="status">
        <Hand size={14} aria-hidden="true" />
        {tn('memory.guards.waitingForYou', waiting)}
        <Button variant="warning" class="ml-auto brightness-125" onclick={review}>{t('memory.guards.review')}</Button>
      </div>
    {/if}

    <div class="flex flex-col gap-2.5 border-t border-line p-2.5">
      <MemoryKindCard kind="notes" {agentKey} {root} {repoName} />
      <MemoryKindCard kind="scripts" {agentKey} {root} {repoName} />
    </div>

    <footer class="flex gap-2 border-t border-line px-4 pt-2.5 pb-3">
      <Button class="flex-1" loading={checking} disabled={guards.paused || nothingToSave} onclick={() => void checkAgain()}>
        {#snippet leading()}<RefreshCw size={14} aria-hidden="true" />{/snippet}
        {checking ? t('memory.guards.checking') : t('memory.guards.check')}
      </Button>

      {#if guards.paused}
        <Button variant="primary" class="flex-1" onclick={() => void memoryChat.setPaused(agentKey, false)}>
          {#snippet leading()}<Play size={14} aria-hidden="true" />{/snippet}
          {t('memory.guards.resume')}
        </Button>
      {:else}
        <Button variant="warning" class="flex-1" disabled={nothingToSave} onclick={() => void memoryChat.setPaused(agentKey, true)}>
          {#snippet leading()}<Pause size={14} aria-hidden="true" />{/snippet}
          {t('memory.guards.pause')}
        </Button>
      {/if}
    </footer>

    {#if guards.paused}
      <p class="-mt-1 px-4 pb-3 text-xs text-fg-3">{t('memory.guards.pausedNote')}</p>
    {/if}
  </div>
{/if}

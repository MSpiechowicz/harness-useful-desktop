<script lang="ts">
  import Hand from '@lucide/svelte/icons/hand';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { heldCounts } from '$lib/chat/memory-held';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { memoryHeld } from '$lib/stores/memoryHeld.svelte';

  import MemoryHeldItem from './MemoryHeldItem.svelte';

  interface Props {
    /** The key of the chat's agent. */
    agentKey: string;
  }

  let { agentKey }: Props = $props();

  const memory = $derived(memoryChat.get(agentKey));
  const items = $derived(memory ? memoryHeld.of(memory.key) : []);
  const counts = $derived(heldCounts(items));
  const saving = $derived(items.some((item) => memoryHeld.busy.has(item.heldId)));

  // What the card says the agent wants: "1 note", "1 script", or "1 note and 2 scripts".
  const wants = $derived.by(() => {
    const notes = tn('memory.guards.waiting.notes', counts.notes);
    const scripts = tn('memory.guards.waiting.scripts', counts.scripts);

    if (counts.notes > 0 && counts.scripts > 0) {
      return t('memory.guards.waiting.both', { notes, scripts });
    }

    return counts.notes > 0 ? notes : scripts;
  });

  /** Saves the waiting items one after the other, so a failure on one does not hide the rest. */
  async function saveAll(): Promise<void> {
    for (const item of items) {
      await memoryHeld.save(item);
    }
  }
</script>

<!-- The saves an agent holds for review: nothing is written until the user presses Save. Pinned after the last turn while any wait. -->
<section aria-label={t('memory.guards.waiting.label')} data-memory-waiting class="chat-radius overflow-hidden border border-warning/40 bg-surface-1 shadow-float">
  <header class="flex items-center gap-3 border-b border-line px-4 py-3">
    <Hand size={ICON.md} class="shrink-0 text-fg-2" aria-hidden="true" />

    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <h3 class="text-base font-semibold text-fg">{t('memory.guards.waiting.title', { what: wants })}</h3>
      <p class="text-xs text-fg-3">{t('memory.guards.waiting.subtitle')}</p>
    </div>

    <Button disabled={saving} onclick={() => void saveAll()}>{t('memory.guards.waiting.saveAll')}</Button>
  </header>

  {#each items as item (item.heldId)}
    <MemoryHeldItem {item} />
  {/each}
</section>

<script lang="ts">
  import Hand from '@lucide/svelte/icons/hand';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { heldSummary } from '$lib/chat/memory-held';
  import { t } from '$lib/i18n/i18n.svelte';
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
  const saving = $derived(items.some((item) => memoryHeld.busy.has(item.heldId)));
  const wants = $derived(heldSummary(items));
</script>

<!-- The saves an agent holds for review: nothing is written until the user presses Save. Pinned after the last turn while any wait. -->
<section aria-label={t('memory.guards.waiting.label')} data-memory-waiting class="chat-radius overflow-hidden border border-warning/40 bg-surface-1 shadow-float">
  <header class="flex items-center gap-3 border-b border-line px-4 py-3">
    <Hand size={ICON.md} class="shrink-0 text-fg-2" aria-hidden="true" />

    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <h3 class="text-base font-semibold text-fg">{t('memory.guards.waiting.title', { what: wants })}</h3>
      <p class="text-xs text-fg-3">{t('memory.guards.waiting.subtitle')}</p>
    </div>

    <Button disabled={saving} onclick={() => void memoryHeld.saveAll(items)}>{t('memory.guards.waiting.saveAll')}</Button>
  </header>

  {#each items as item (item.heldId)}
    <MemoryHeldItem {item} />
  {/each}
</section>

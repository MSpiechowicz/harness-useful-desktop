<script lang="ts">
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';

  import { prefs } from '$lib/stores/prefs.svelte';

  const shown = $derived(prefs.value?.showThinking ?? true);
</script>

<!-- Shows or hides the agent's thought process in every chat; the same switch lives in Settings. -->
<button
  type="button"
  aria-pressed={shown}
  aria-label="Show thought process"
  title={shown ? 'Hide the thought process in chats' : 'Show the thought process in chats'}
  onclick={() => void prefs.update({ showThinking: !shown })}
  class="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-sm text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg"
>
  {#if shown}<Eye size={16} aria-hidden="true" class="shrink-0" />{:else}<EyeOff size={16} aria-hidden="true" class="shrink-0" />{/if}
  Thinking
</button>

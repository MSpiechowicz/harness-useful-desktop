<script lang="ts">
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';

  import { prefs } from '$lib/stores/prefs.svelte';

  interface Props {
    /** The preference this button flips; Settings has the same switch. */
    pref: 'showThinking' | 'showMessageUsage';
    label: string;
    /** Accessible name: what is shown while the button is pressed. */
    aria: string;
    /** Tooltip while shown, and while hidden. */
    hideTitle: string;
    showTitle: string;
  }

  let { pref, label, aria, hideTitle, showTitle }: Props = $props();

  const shown = $derived(prefs.value?.[pref] ?? true);
</script>

<!-- Shows or hides one part of every chat (the thought process, the cost under replies). -->
<button
  type="button"
  aria-pressed={shown}
  aria-label={aria}
  title={shown ? hideTitle : showTitle}
  onclick={() => void prefs.update({ [pref]: !shown })}
  class="inline-flex h-control items-center gap-1.5 rounded-md px-2 text-sm text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg"
>
  {#if shown}<Eye size={16} aria-hidden="true" class="shrink-0" />{:else}<EyeOff size={16} aria-hidden="true" class="shrink-0" />{/if}
  {label}
</button>

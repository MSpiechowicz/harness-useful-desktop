<script lang="ts">
  import Brain from '@lucide/svelte/icons/brain';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  interface Props {
    text: string;
    redacted: boolean;
    streaming: boolean;
  }

  let { text, redacted, streaming }: Props = $props();

  // Open while the model is thinking, collapsed once it is done, unless the user chose otherwise.
  let chosen = $state<boolean | null>(null);
  const open = $derived(chosen ?? streaming);
</script>

<div class="rounded-lg border border-line bg-surface-1/50">
  <button
    type="button"
    aria-expanded={open}
    class="flex w-full items-center gap-2 rounded-lg px-3 py-1.5 text-left text-sm text-fg-3 transition-colors hover:text-fg-2"
    onclick={() => (chosen = !open)}
  >
    <Brain size={16} aria-hidden="true" />
    <span class={cx(streaming && 'sheen')}>{streaming ? 'Thinking…' : 'Thought process'}</span>
    <ChevronRight size={16} aria-hidden="true" class={cx('ml-auto transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} />
  </button>

  {#if open}
    <div transition:slide={{ duration: scaled(180) }} class="overflow-hidden">
      <p class="px-3 pb-3 text-sm leading-relaxed break-words whitespace-pre-wrap text-fg-2">
        {redacted ? 'The provider keeps this reasoning private.' : text}{#if streaming}<span class="caret" aria-hidden="true"></span>{/if}
      </p>
    </div>
  {/if}
</div>

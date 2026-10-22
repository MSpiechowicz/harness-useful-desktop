<script lang="ts">
  import Sparkles from '@lucide/svelte/icons/sparkles';

  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import ChatRow from './ChatRow.svelte';

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

<ChatRow {open} ontoggle={() => (chosen = !open)} tone="think">
  {#snippet icon()}
    <!-- The spinner takes the brain's place at the same size, so the row does not shift when the thinking ends. -->
    {#if streaming}<Spinner size={16} />{:else}<Sparkles size={16} aria-hidden="true" />{/if}
  {/snippet}

  {#snippet title()}
    <span class={cx(streaming ? 'sheen' : 'text-fg-3')}>{streaming ? t('chat.thinking.active') : t('chat.thinking.done')}</span>
  {/snippet}

  {#snippet body()}
    <p class="mx-4 my-3 text-sm leading-relaxed break-words whitespace-pre-wrap text-fg-2">
      {redacted ? t('chat.thinking.redacted') : text}{#if streaming}<span class="caret" aria-hidden="true"></span>{/if}
    </p>
  {/snippet}
</ChatRow>

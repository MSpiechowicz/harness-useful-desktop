<script lang="ts">
  import ArrowUp from '@lucide/svelte/icons/arrow-up';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    placeholder: string;
    disabled?: boolean;
    /** The height of the area in px, so its top edge lines up with the top edge of the chat composer; unset, it is as tall as its content. */
    height: number | null;
    /** The chat composer card's height in px: the box ends where that card ends, so the two read as one row. */
    cardHeight: number | null;
    /** Sends the text; true when it went out, so the field can clear. */
    onsend: (text: string) => Promise<boolean>;
  }

  let { placeholder, disabled = false, height, cardHeight, onsend }: Props = $props();



  let draft = $state('');
  let sending = $state(false);
  let field: HTMLTextAreaElement | undefined = $state();

  const ready = $derived(draft.trim() !== '' && !sending && !disabled);

  // Without the composer's height to match, the field grows with the text up to a cap (`--message-field-max`); one line is as
  // tall as a button. With it, the field fills the box and scrolls.
  $effect(() => {
    void draft;

    if (field && cardHeight === null) {
      field.style.height = 'auto';
      field.style.height = `${field.scrollHeight}px`;
    }
  });

  async function send(): Promise<void> {
    const text = draft.trim();
    if (!ready) {
      return;
    }

    sending = true;
    const sent = await onsend(text);
    sending = false;

    // Text typed while the message was on its way stays in the field.
    if (sent && draft.trim() === text) {
      draft = '';
    }

    field?.focus();
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      void send();
    }
  }
</script>

<!-- The box is the chat composer's card mirrored: the same border, radius and text, and the same top and bottom edge as that card
     (the area starts where the card starts and the box is as tall as the card). The field fills the whole box, so the text and its
     scrollbar run to the box's edges, and the send button is a full-width row under it, in the room the composer leaves below its card. -->
<div data-subagent-message class="flex shrink-0 flex-col panel-pad" style:height={height === null ? undefined : `${height}px`}>
  <div
    class="flex shrink-0 flex-col border-card chat-radius border-line-strong bg-surface-1 bg-clip-padding transition-[border-color] duration-[var(--dur-fast)] focus-within:border-[color-mix(in_oklch,var(--color-fg)_28%,var(--surface-1))]"
    style:height={cardHeight === null ? undefined : `${cardHeight}px`}
  >
    <textarea
      bind:this={field}
      bind:value={draft}
      {onkeydown}
      {placeholder}
      {disabled}
      rows="1"
      spellcheck="true"
      aria-label={t('subagents.message.label')}
      class="block min-h-14 w-full flex-1 resize-none bg-transparent px-4 py-3.5 text-[15px] leading-6 text-fg outline-none placeholder:text-fg-3 disabled:cursor-not-allowed disabled:opacity-60 {cardHeight === null ? 'max-h-[var(--message-field-max)]' : ''}"
    ></textarea>
  </div>

  <Button variant="primary" class="mt-2.5 w-full shrink-0" disabled={!ready} onclick={() => void send()}>
    {#snippet leading()}<ArrowUp size={18} strokeWidth={2.5} aria-hidden="true" />{/snippet}
    {t('subagents.message.send')}
  </Button>

</div>

<script lang="ts">
  import Check from '@lucide/svelte/icons/check';

  import Badge from '$lib/components/Badge.svelte';
  import { optionLabel } from '$lib/chat/question-title';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { visibleText } from '$lib/utils/display-text';

  interface Props {
    /** The one-based number the digit key picks. */
    number: number;
    label: string;
    detail?: string | null;
    /** The agent says it recommends this one, also when its label does not. */
    recommended?: boolean;
    selected?: boolean;
    /** Another option of the question is picked, so the recommendation only keeps its badge. */
    otherPicked?: boolean;
    button?: HTMLButtonElement;
    onclick: () => void;
    onkeydown: (event: KeyboardEvent) => void;
  }

  let { number, label, detail, recommended = false, selected = false, otherPicked = false, button = $bindable(), onclick, onkeydown }: Props = $props();

  const shown = $derived(optionLabel(visibleText(label)));
  const isRecommended = $derived(recommended || shown.recommended);
  const leads = $derived(isRecommended && !otherPicked);

  const chip = $derived.by(() => {
    if (selected) {
      return 'bg-[image:var(--brand-fill)] text-on-accent shadow-float';
    }

    return leads ? 'bg-accent-wash text-accent-text' : 'bg-surface-4 text-fg group-hover:bg-accent-wash group-hover:text-accent-text';
  });
</script>

<button
  bind:this={button}
  type="button"
  aria-pressed={selected}
  class={cx(
    'group flex w-full items-start gap-3 rounded-[10px] border px-3.5 py-3 text-left transition-colors duration-[var(--dur-fast)]',
    selected ? 'border-accent bg-accent-wash' : leads ? 'border-accent/50 bg-surface-2 hover:bg-surface-3' : 'border-line bg-surface-2 hover:bg-surface-3',
  )}
  {onclick}
  {onkeydown}
>
  <!-- No colour transition: a gradient cannot fade, so the chip would flash dark between a tick and a number. -->
  <span class={cx('grid size-7 shrink-0 place-items-center rounded-[8px] text-sm font-semibold', chip)}>
    {#if selected}
      <span class="sr-only">{number}</span>
      <Check size={16} strokeWidth={3} aria-hidden="true" class="block translate-y-[0.33px]" />
    {:else}
      {number}
    {/if}
  </span>

  <span class="flex min-w-0 flex-1 flex-col gap-0.5">
    <span class="flex w-full items-start gap-2">
      <span class="min-w-0 flex-1 text-base font-medium text-fg">{shown.text}</span>
      {#if isRecommended}
        <Badge tone="accent" class="mt-0.5 shrink-0">{t('chat.request.recommended')}</Badge>
      {/if}
    </span>
    {#if detail}<span class="text-sm text-fg-2">{visibleText(detail)}</span>{/if}
  </span>
</button>

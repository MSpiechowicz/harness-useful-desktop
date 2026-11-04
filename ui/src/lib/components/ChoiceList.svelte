<script lang="ts">
  import Check from '@lucide/svelte/icons/check';

  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import type { Choice } from './change-list';

  /**
   * A labelled single choice among a few rows, each with an icon, a title and a quieter detail line, e.g. where to send a
   * review or how to finish one. It knows nothing about what the choices mean.
   */
  interface Props {
    /** The question above the rows. */
    label: string;
    choices: Choice[];
    value: string;
    class?: string;
  }

  let { label, choices, value = $bindable(), class: className }: Props = $props();

  const labelId = uid('choice');
</script>

<div class={className}>
  <p class="mb-2 text-sm font-medium text-fg-2" id={labelId}>{label}</p>
  <div class="flex flex-col gap-1" role="radiogroup" aria-labelledby={labelId}>
    {#each choices as choice (choice.value)}
      {@const selected = choice.value === value}
      {@const Icon = choice.icon}
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        class={cx(
          'flex w-full items-center gap-3 rounded-md border px-3 py-2 text-left transition-colors duration-[var(--dur-fast)]',
          selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
        )}
        onclick={() => (value = choice.value)}
      >
        {#if Icon}<Icon size={16} aria-hidden="true" class="shrink-0 text-fg-3" />{/if}
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="truncate text-sm font-medium text-fg">{choice.title}</span>
          {#if choice.detail}<span class="truncate text-xs text-fg-3">{choice.detail}</span>{/if}
        </span>
        <Check size={16} aria-hidden="true" class={cx('shrink-0 text-accent-text', !selected && 'invisible')} />
      </button>
    {/each}
  </div>
</div>

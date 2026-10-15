<script lang="ts">
  import Check from '@lucide/svelte/icons/check';

  import { cx } from '$lib/utils/cx';

  interface Props {
    checked: boolean;
    /** Accessible name; the box has no visible text of its own. */
    label: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked, label, disabled = false, onchange }: Props = $props();
</script>

<!-- A 36px hit area (the control height) around an 18px box, so rows of checkboxes are as easy to hit as any control. -->
<button
  type="button"
  role="checkbox"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange?.(!checked)}
  class="group grid size-control shrink-0 place-items-center rounded-md outline-none disabled:cursor-not-allowed disabled:opacity-50"
>
  <span
    class={cx(
      'grid size-[18px] place-items-center rounded-[5px] border transition-colors duration-[var(--dur-fast)] group-focus-visible:ring-2 group-focus-visible:ring-accent/50',
      checked ? 'border-transparent bg-[image:var(--brand-fill)] text-on-accent' : 'border-line-strong bg-surface-2 group-enabled:group-hover:bg-surface-3',
    )}
  >
    {#if checked}
      <Check size={14} strokeWidth={3} aria-hidden="true" />
    {/if}
  </span>
</button>

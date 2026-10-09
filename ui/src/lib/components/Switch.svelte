<script lang="ts">
  import { cx } from '$lib/utils/cx';

  interface Props {
    checked?: boolean;
    /** Accessible name; the switch has no visible text of its own. */
    label: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked = $bindable(false), label, disabled = false, onchange }: Props = $props();

  function toggle(): void {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={toggle}
  class={cx(
    'relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border border-line-strong p-0.5 transition-colors duration-[var(--dur-base)] disabled:cursor-not-allowed disabled:opacity-50',
    checked ? 'border-transparent bg-[image:var(--brand-fill)]' : 'bg-surface-4',
  )}
>
  <span
    class={cx(
      'size-[18px] rounded-full shadow-raised transition-transform duration-[var(--dur-slow)] ease-[var(--ease-spring)]',
      checked ? 'translate-x-5 bg-on-accent' : 'translate-x-0 bg-fg-2',
    )}
  ></span>
</button>

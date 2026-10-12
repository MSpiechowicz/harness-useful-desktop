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
    // Pixel sizes, not rem: the app's root font is 14px (h-6 = 21px), and the track must be a fixed 24px so the 18px
    // knob centres with 2px on each side, no subpixel padding to split asymmetrically under the inset highlight.
    'relative inline-flex h-[24px] w-[44px] shrink-0 items-center rounded-full border border-line-strong p-[2px] transition-colors duration-[var(--dur-base)] disabled:cursor-not-allowed disabled:opacity-50',
    checked ? 'border-transparent bg-[image:var(--brand-fill)]' : 'bg-surface-4',
  )}
>
  <span
    class={cx(
      'size-[18px] rounded-full shadow-raised transition-transform duration-[var(--dur-slow)] ease-[var(--ease-spring)]',
      // Pixel translate, not rem: the on-state must move the 18px knob exactly 20px (42px inner width − 2×2px padding − 18px knob)
      // so it parks 2px from the right edge, mirroring the 2px gap on the left in the off-state.
      checked ? 'translate-x-[20px] bg-on-accent' : 'translate-x-0 bg-fg-2',
    )}
  ></span>
</button>

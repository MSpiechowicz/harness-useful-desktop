<script lang="ts">
  import { cx } from '$lib/utils/cx';

  interface Props {
    size?: number;
    /** Accessible name; omit when the spinner is decorative next to visible text. */
    label?: string;
    class?: string;
  }

  let { size = 16, label, class: className }: Props = $props();
</script>

<svg
  class={cx('keep-motion shrink-0 animate-spin', className)}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  role={label ? 'status' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
>
  <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-opacity="0.2" stroke-width="2" />
  <path d="M22 12a10 10 0 0 0-10-10" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
</svg>

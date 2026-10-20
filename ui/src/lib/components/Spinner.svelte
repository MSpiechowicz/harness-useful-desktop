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
  class={cx('shrink-0', className)}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  role={label ? 'status' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
>
  <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-opacity="0.2" stroke-width="2" />
  <!-- Only the arc turns, about the circle's own centre; the track stays put, so the ring cannot wobble sideways
       when the pill around it sits on a fractional pixel. -->
  <path class="keep-motion animate-spin" style="transform-origin: 12px 12px" d="M22 12a10 10 0 0 0-10-10" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
</svg>

<script lang="ts">
  import { cx } from '$lib/utils/cx';

  interface Props {
    width?: string;
    height?: string;
    class?: string;
  }

  let { width = '100%', height = '1rem', class: className }: Props = $props();
</script>

<div
  class={cx('shimmer relative overflow-hidden rounded-md bg-surface-3', className)}
  style:width
  style:height
  aria-hidden="true"
></div>

<style>
  .shimmer::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, var(--hover-wash), transparent);
    animation: shimmer 1.6s var(--ease-in-out) infinite;
  }
</style>

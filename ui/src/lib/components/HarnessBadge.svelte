<script lang="ts">
  import type { HarnessInfo } from '$lib/ipc/bindings/HarnessInfo';
  import { cx } from '$lib/utils/cx';

  interface Props {
    info: Pick<HarnessInfo, 'id' | 'shortName'>;
    size?: 'sm' | 'md' | 'lg';
    class?: string;
  }

  let { info, size = 'md', class: className }: Props = $props();

  const SIZES = {
    sm: 'h-6 min-w-6 rounded-sm px-1 text-[10px]',
    md: 'h-9 min-w-9 rounded-md px-1.5 text-xs',
    lg: 'h-12 min-w-12 rounded-lg px-2 text-sm',
  } as const;
  // A plain wash, no outline: a translucent hairline reads as a muddy double edge on the rounded corners.
  const COLORS = { omp: 'bg-omp-wash text-omp', pi: 'bg-pi-wash text-pi' } as const;
</script>

<!-- Initials in the harness accent; decorative, the harness name always appears next to it. -->
<span
  aria-hidden="true"
  class={cx('inline-grid shrink-0 place-items-center font-semibold tracking-wide', SIZES[size], COLORS[info.id], className)}
>
  {info.shortName}
</span>

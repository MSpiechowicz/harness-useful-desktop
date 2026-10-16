<script lang="ts">
  import type { HarnessInfo } from '$lib/ipc/bindings/HarnessInfo';
  import { cx } from '$lib/utils/cx';

  import { harnessMark } from './harness-mark';

  interface Props {
    info: Pick<HarnessInfo, 'id' | 'shortName'>;
    size?: 'xs' | 'sm' | 'tile' | 'bar' | 'md' | 'lg';
    class?: string;
  }

  let { info, size = 'md', class: className }: Props = $props();

  const SIZES = {
    /** As wide as the three-letter "OMP", so a two-letter "PI" beside it is not a smaller badge. */
    xs: 'h-[18px] min-w-[34px] rounded-sm px-1 text-[10px]',
    sm: 'h-6 w-9 rounded-sm text-[10px]',
    /** A square the size of a control, as wide as the tile a product mark sits on. */
    tile: 'size-control rounded-md text-[10px]',
    /** The height of the controls in a view's top bar. */
    bar: 'h-control min-w-control rounded-md px-2 text-xs',
    md: 'h-control min-w-control rounded-md px-1.5 text-xs',
    /** A fixed width, wide enough for "OMP", so every harness has the same badge. */
    lg: 'h-12 w-14 rounded-lg text-sm',
  } as const;
  // A plain wash, no outline: a translucent hairline reads as a muddy double edge on the rounded corners.
  const COLORS: Record<string, string> = {
    omp: 'bg-omp-wash text-omp',
    pi: 'bg-pi-wash text-pi',
    opencode: 'bg-opencode-wash text-opencode',
  };
  /** The badge is sized for "OMP": a longer name ("OpenCode") is reduced to a short mark, "OC", and the full name stays in the text beside it. */
  const mark = $derived(harnessMark(info.shortName));
  /** A harness without accent tokens of its own (yet) gets the neutral wash. */
  const NEUTRAL = 'bg-surface-3 text-fg-2';
</script>

<!-- Initials in the harness accent; decorative, the harness name always appears next to it. -->
<span
  aria-hidden="true"
  class={cx('inline-grid shrink-0 place-items-center font-semibold tracking-wide', SIZES[size], COLORS[info.id] ?? NEUTRAL, className)}
>
  {mark}
</span>

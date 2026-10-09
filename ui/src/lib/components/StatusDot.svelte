<script lang="ts">
  import { cx } from '$lib/utils/cx';

  type Tone = 'success' | 'warning' | 'danger' | 'neutral';

  interface Props {
    tone: Tone;
    class?: string;
  }

  let { tone, class: className }: Props = $props();

  const TONES: Record<Tone, string> = {
    // `text-*` carries the same color into `currentColor`, which the breathing halo (.breathe) glows with.
    success: 'bg-success text-success',
    warning: 'bg-warning text-warning',
    danger: 'bg-danger text-danger',
    neutral: 'bg-fg-3 text-fg-3',
  };
</script>

<span class={cx('inline-block size-2 shrink-0 rounded-full', TONES[tone], className)} aria-hidden="true"></span>

<script lang="ts">
  import { cx } from '$lib/utils/cx';

  type Tone = 'success' | 'warning' | 'danger' | 'neutral';

  interface Props {
    tone: Tone;
    class?: string;
  }

  let { tone, class: className }: Props = $props();

  const TONES: Record<Tone, string> = {
    success: 'bg-success',
    warning: 'bg-warning',
    danger: 'bg-danger',
    neutral: 'bg-fg-3',
  };
</script>

<span class={cx('inline-block size-2 shrink-0 rounded-full', TONES[tone], className)} aria-hidden="true"></span>

<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  type Tone = 'plain' | 'think' | 'run' | 'info' | 'success' | 'danger';

  interface Props {
    /** The tint: plain grey, the agent thinking, a tool running (accent), project memory (info), done (success) or failed (danger). */
    tone?: Tone;
    /** `card` is the tile of a chat card header. `md` and `lg` are the tokens `--tile-md` and `--tile-lg`: a step of a list, the live card of a subagent. */
    size?: 'card' | 'md' | 'lg';
    class?: string;
    children: Snippet;
  }

  let { tone = 'plain', size = 'card', class: className, children }: Props = $props();

  const TONES: Record<Tone, string> = {
    plain: 'bg-surface-2 text-fg-2',
    think: 'bg-think-wash text-think',
    run: 'bg-accent-wash text-accent-text',
    info: 'bg-info-wash text-info',
    success: 'bg-success-wash text-success',
    danger: 'bg-danger-wash text-danger',
  };

  const SIZES = {
    card: 'size-7 rounded-[8px]',
    md: 'size-[var(--tile-md)] rounded-sm transition-colors duration-[var(--dur-slow)]',
    lg: 'size-[var(--tile-lg)] rounded-[var(--tile-lg-radius)] transition-colors duration-[var(--dur-slow)]',
  };
</script>

<!-- The square that holds a 16px icon: the same shape and tints wherever a row has an icon (chat cards, steps, the subagent panel). -->
<span class={cx('grid shrink-0 place-items-center', SIZES[size], TONES[tone], className)}>{@render children()}</span>

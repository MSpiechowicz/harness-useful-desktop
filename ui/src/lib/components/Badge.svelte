<script lang="ts">
  import type { Snippet } from 'svelte';

  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { cx } from '$lib/utils/cx';

  type Tone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';

  interface Props {
    tone?: Tone;
    /** Harness badges take their color from the per-harness accent tokens and ignore `tone`. */
    harness?: HarnessId;
    dot?: boolean;
    /** Quieter than the filled badge: no background, a thin ring and the tone's text. For a state that is the same as another one seen from a different side. Harness badges stay filled. */
    outline?: boolean;
    /** `lg` is for a card's own status row, where the default reads too small beside body text. */
    size?: 'md' | 'lg';
    class?: string;
    children: Snippet;
  }

  let { tone = 'neutral', harness, dot = false, outline = false, size = 'md', class: className, children }: Props = $props();

  const TONES: Record<Tone, string> = {
    neutral: 'bg-surface-3 text-fg-2',
    accent: 'bg-accent-wash text-accent-text',
    success: 'bg-success-wash text-success',
    warning: 'bg-warning-wash text-warning',
    danger: 'bg-danger-wash text-danger',
    info: 'bg-info-wash text-info',
  };
  const OUTLINES: Record<Tone, string> = {
    neutral: 'text-fg-2 ring-1 ring-inset ring-fg-2/35',
    accent: 'text-accent-text ring-1 ring-inset ring-accent-text/35',
    success: 'text-success ring-1 ring-inset ring-success/35',
    warning: 'text-warning ring-1 ring-inset ring-warning/35',
    danger: 'text-danger ring-1 ring-inset ring-danger/35',
    info: 'text-info ring-1 ring-inset ring-info/35',
  };
  const HARNESSES: Record<HarnessId, string> = {
    omp: 'bg-omp-wash text-omp',
    pi: 'bg-pi-wash text-pi',
    opencode: 'bg-opencode-wash text-opencode',
    claude: 'bg-claude-wash text-claude',
    codex: 'bg-codex-wash text-codex',
  };

  const SIZES = {
    md: 'gap-1.5 px-2 py-0.5 text-xs leading-4',
    lg: 'h-7 gap-2 px-3 text-sm leading-5',
  } as const;

  const colors = $derived(harness ? HARNESSES[harness] : outline ? OUTLINES[tone] : TONES[tone]);
</script>

<span
  class={cx(
    'inline-flex items-center rounded-full font-medium tracking-wide whitespace-nowrap',
    SIZES[size],
    colors,
    className,
  )}
>
  {#if dot}
    <span class="size-1.5 rounded-full bg-current" aria-hidden="true"></span>
  {/if}
  {@render children()}
</span>

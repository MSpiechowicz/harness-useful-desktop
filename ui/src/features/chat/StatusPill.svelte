<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  interface Props {
    tone: 'ok' | 'run' | 'error' | 'idle' | 'info';
    label: string;
    icon: Snippet;
    /** Room for a longer label (two counts side by side) before it is cut. */
    wide?: boolean;
  }

  let { tone, label, icon, wide = false }: Props = $props();

  const TONES = { ok: 'bg-success-wash text-success', run: 'bg-accent-wash text-accent-text', error: 'bg-danger-wash text-danger', idle: 'bg-surface-2 text-fg-3', info: 'bg-info-wash text-info' };
</script>

<!-- Hugs its content: icon and label are centred as one group with even padding. -->
<span class={cx('inline-flex h-7 items-center gap-1.5 rounded-full px-3 text-sm leading-5 font-medium', wide ? 'max-w-56' : 'max-w-36', TONES[tone])}>
  <span class="grid size-[16px] shrink-0 place-items-center">{@render icon()}</span>
  <span class="truncate" title={label}>{label}</span>
</span>

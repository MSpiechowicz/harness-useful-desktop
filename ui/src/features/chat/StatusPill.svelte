<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  interface Props {
    tone: 'ok' | 'run' | 'error' | 'idle';
    label: string;
    icon: Snippet;
  }

  let { tone, label, icon }: Props = $props();

  const TONES = { ok: 'bg-success-wash text-success', run: 'bg-accent-wash text-accent-text', error: 'bg-danger-wash text-danger', idle: 'bg-surface-2 text-fg-3' };
</script>

<!-- A fixed width, so the icon (spinner, check, clock) and the label start at the same x on every row. -->
<span class={cx('grid h-6 w-30 grid-cols-[16px_minmax(0,1fr)] items-center gap-x-1.5 rounded-full pr-2.5 pl-2 text-xs font-medium', TONES[tone])}>
  <span class="grid size-4 place-items-center">{@render icon()}</span>
  <span class="truncate" title={label}>{label}</span>
</span>

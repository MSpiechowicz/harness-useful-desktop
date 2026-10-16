<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  interface Props {
    tone: 'ok' | 'run' | 'error' | 'idle' | 'info';
    label: string;
    icon: Snippet;
  }

  let { tone, label, icon }: Props = $props();

  const TONES = { ok: 'bg-success-wash text-success', run: 'bg-accent-wash text-accent-text', error: 'bg-danger-wash text-danger', idle: 'bg-surface-2 text-fg-3', info: 'bg-info-wash text-info' };
</script>

<!-- Hugs its content: icon and label are centred as one group with even padding. -->
<span class={cx('inline-flex h-7 max-w-36 items-center gap-1.5 rounded-full px-3 text-sm leading-5 font-medium', TONES[tone])}>
  <span class="grid size-[16px] shrink-0 place-items-center">{@render icon()}</span>
  <span class="truncate" title={label}>{label}</span>
</span>

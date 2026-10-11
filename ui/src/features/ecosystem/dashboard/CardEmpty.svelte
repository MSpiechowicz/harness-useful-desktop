<script lang="ts">
  import type { IconComponent } from '$lib/components/icon';
  import { cx } from '$lib/utils/cx';

  interface Props {
    icon: IconComponent;
    /** The state in one sentence: "No budgets are set." */
    title: string;
    /** What to do about it, when there is something. */
    hint?: string;
    /** Good news (nothing failed) reads green; everything else uses the accent. */
    tone?: 'accent' | 'success';
  }

  let { icon: Icon, title, hint, tone = 'accent' }: Props = $props();
</script>

<!-- The one empty state every dashboard card uses: an icon tile beside a sentence and an optional hint. -->
<div class="flex items-center gap-3 py-1" role="status">
  <span class={cx('grid size-9 shrink-0 place-items-center rounded-md border border-line bg-surface-2', tone === 'success' ? 'text-success' : 'text-accent-text')}>
    <Icon size={18} strokeWidth={1.8} aria-hidden="true" />
  </span>
  <div class="flex min-w-0 flex-col">
    <p class="text-sm text-fg-2">{title}</p>
    {#if hint}<p class="text-xs text-fg-3">{hint}</p>{/if}
  </div>
</div>

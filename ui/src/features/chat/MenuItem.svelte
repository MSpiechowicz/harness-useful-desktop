<script lang="ts">
  import Check from '@lucide/svelte/icons/check';

  import { cx } from '$lib/utils/cx';

  interface Props {
    label: string;
    detail?: string | null;
    selected?: boolean;
    onselect: () => void;
  }

  let { label, detail = null, selected = false, onselect }: Props = $props();
</script>

<!-- A radio-style row of a picker; `data-menu-item` lets the popover drive arrow-key navigation. -->
<button
  type="button"
  role="menuitemradio"
  aria-checked={selected}
  data-menu-item
  class={cx(
    'flex w-full items-center gap-2.5 rounded-sm px-2.5 py-1.5 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover',
    selected ? 'text-fg' : 'text-fg-2',
  )}
  onclick={onselect}
>
  <Check size={14} aria-hidden="true" class={cx('shrink-0 text-accent-text', !selected && 'invisible')} />
  <span class="min-w-0 flex-1 truncate">{label}</span>
  {#if detail}<span class="tabular shrink-0 text-xs text-fg-3">{detail}</span>{/if}
</button>

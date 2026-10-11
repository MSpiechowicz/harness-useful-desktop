<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import type { Component } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import type { ModeTone } from './approval-modes';

  interface Props {
    label: string;
    description: string;
    icon: Component<{ size?: number; class?: string; 'aria-hidden'?: 'true' }>;
    tone: ModeTone | 'neutral';
    selected: boolean;
    /** The mode the session runs with now. */
    current: boolean;
    onselect: () => void;
  }

  let { label, description, icon: Icon, tone, selected, current, onselect }: Props = $props();

  const TILE: Record<ModeTone | 'neutral', string> = {
    neutral: 'bg-hover text-fg-2',
    success: 'bg-success-wash text-success',
    info: 'bg-info-wash text-info',
    warning: 'bg-warning-wash text-warning',
  };
</script>

<!-- One approval mode in the chooser; `data-menu-item` lets the popover drive arrow-key navigation. -->
<button
  type="button"
  role="menuitemradio"
  aria-checked={selected}
  data-menu-item
  class={cx(
    'flex w-full items-center gap-3 rounded-md px-2.5 py-2 text-left transition-colors duration-[var(--dur-fast)] focus-visible:bg-hover',
    selected ? 'bg-accent-wash' : 'hover:bg-hover active:bg-press',
  )}
  onclick={onselect}
>
  <span class={cx('grid size-8 shrink-0 place-items-center rounded-md', TILE[tone])}>
    <Icon size={16} aria-hidden="true" />
  </span>

  <span class="flex min-w-0 flex-1 flex-col gap-0.5">
    <span class="flex items-center gap-2 text-sm font-medium text-fg">
      <span class="truncate first-letter:uppercase">{label}</span>
      {#if current}<span class="shrink-0 rounded-full bg-hover px-1.5 py-px text-[11px] font-normal text-fg-3">Current</span>{/if}
    </span>
    <span class="truncate text-xs text-fg-3 first-letter:uppercase">{description}</span>
  </span>

  <!-- The chosen row gets a check, like the model and thinking pickers; the row tint already marks it. -->
  <Check size={16} aria-hidden="true" class={cx('shrink-0 text-accent-text', !selected && 'invisible')} />
</button>

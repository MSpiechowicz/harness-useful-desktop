<script lang="ts" generics="T extends string">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import { cx } from '$lib/utils/cx';

  import type { StripOption } from './strip';

  interface Props {
    options: StripOption<T>[];
    value: T;
    label: string;
    disabled?: boolean;
    onchange?: (value: T) => void;
    class?: string;
  }

  let { options, value = $bindable(), label, disabled = false, onchange, class: className }: Props = $props();
</script>

<div class={cx('relative inline-flex', className)}>
  <select
    bind:value
    aria-label={label}
    {disabled}
    onchange={() => onchange?.(value)}
    class="h-9 min-w-36 appearance-none rounded-md border border-line-strong bg-surface-2 pr-9 pl-3 text-base text-fg shadow-raised transition-colors duration-[var(--dur-fast)] hover:bg-surface-3 disabled:cursor-not-allowed disabled:opacity-50"
  >
    {#each options as option (option.value)}
      <option value={option.value} disabled={option.disabled}>{option.label}</option>
    {/each}
  </select>
  <ChevronDown class="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-fg-3" size={16} aria-hidden="true" />
</div>

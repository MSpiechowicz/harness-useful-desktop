<script lang="ts">
  import Lock from '@lucide/svelte/icons/lock';

  import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';
  import { cx } from '$lib/utils/cx';

  import { describeContainer, secretHint, secretLabel } from './config-tree';
  import Tooltip from './Tooltip.svelte';

  interface Props {
    value: ConfigValue;
    /** Show clipped long text in full. */
    expanded?: boolean;
    class?: string;
  }

  let { value, expanded = false, class: className }: Props = $props();

  const summary = $derived(describeContainer(value));
</script>

<!-- Typed, colored scalar. Secrets only ever render as a lock chip: no value reaches the DOM. -->
{#if value.kind === 'secret'}
  {@const secret = value.value}
  <Tooltip text={secretHint(secret)} side="bottom">
    {#snippet children({ describedby })}
      <span
        aria-describedby={describedby}
        class="inline-flex items-center gap-1.5 rounded-full border border-line-strong bg-surface-3 px-2 py-px text-xs font-medium text-fg-2"
      >
        <Lock size={16} aria-hidden="true" />
        <span aria-hidden="true">{secretLabel(secret)}</span>
      </span>
    {/snippet}
  </Tooltip>
{:else if summary !== null}
  <span class={cx('text-xs text-fg-3', className)}>{summary}</span>
{:else if value.kind === 'text'}
  <span class={cx('font-mono text-success', expanded ? 'break-words whitespace-pre-wrap' : 'truncate', className)}>{value.value}</span>
{:else if value.kind === 'number'}
  <span class={cx('tabular font-mono text-warning', className)}>{value.value}</span>
{:else if value.kind === 'bool'}
  <span class={cx('font-mono text-accent-text', className)}>{String(value.value)}</span>
{:else}
  <span class={cx('font-mono text-fg-3 italic', className)}>null</span>
{/if}

<script lang="ts">
  import { spelledOut } from '../memory/script-format';

  interface Props {
    /** Labelled rows of chips or text; a row without values is left out. */
    rows: { label: string; values: string[]; mono: boolean }[];
  }

  let { rows }: Props = $props();

  const shown = $derived(rows.filter((row) => row.values.length > 0));
</script>

{#if shown.length > 0}
  <dl class="grid grid-cols-[auto_1fr] items-baseline gap-x-4 gap-y-2 text-sm text-fg-3">
    {#each shown as row (row.label)}
      <dt>{row.label}</dt>
      <dd class="flex flex-wrap gap-2">
        {#each row.values as value (value)}
          <span class={row.mono ? 'rounded-sm border border-line px-1.5 py-0.5 font-mono text-xs text-fg-2' : 'rounded-full bg-surface-3 px-2.5 py-0.5 text-xs text-fg-2'}>{spelledOut(value)}</span>
        {/each}
      </dd>
    {/each}
  </dl>
{/if}

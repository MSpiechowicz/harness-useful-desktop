<script lang="ts">
  import { cx } from '$lib/utils/cx';

  import type { FactRow } from './facts';
  import SectionCard from './SectionCard.svelte';
  import StatusDot from './StatusDot.svelte';

  interface Props {
    heading: string;
    rows: FactRow[];
    /** Without the card around it: see `SectionCard`. */
    flat?: boolean;
  }

  let { heading, rows, flat = false }: Props = $props();

  /** How many characters at the end of a long value stay visible when its middle is cut. */
  const TAIL = 12;

  const TONES = { warning: 'text-warning', danger: 'text-danger' } as const;

  function colour(fact: FactRow): string {
    if (fact.on) {
      return 'text-success';
    }

    return fact.tone ? TONES[fact.tone] : 'text-fg';
  }
</script>

{#if rows.length > 0}
  <SectionCard {heading} {flat}>
    <dl class="flex flex-col">
      {#each rows as fact (fact.label)}
        <!-- Every row is the label on the left and the value on the right, on one line. A long value is shortened in the
             middle, so both its start and its end (the version, the file name) stay readable; the tooltip has all of it. -->
        <div class="flex justify-between gap-3 py-1 text-sm">
          <dt class="shrink-0 text-fg-2">{fact.label}</dt>
          <dd class={cx('flex min-w-0 items-center justify-end gap-1.5 text-right', colour(fact))} title={fact.title ?? fact.value}>
            {#if fact.on}<StatusDot tone="success" beforeText />{/if}
            {#if fact.cutStart}
              <!-- Right to left puts the ellipsis at the start; the isolate keeps the path itself reading left to right. -->
              <span class="min-w-0 truncate" dir="rtl"><bdi>{fact.value}</bdi></span>
            {:else if fact.mono && fact.value.length > TAIL}
              <span class="flex min-w-0 font-mono text-xs leading-5"><span class="truncate">{fact.value.slice(0, -TAIL)}</span><span class="shrink-0">{fact.value.slice(-TAIL)}</span></span>
            {:else}
              <span class={cx('truncate', fact.mono && 'font-mono text-xs leading-5')}>{fact.value}</span>
            {/if}
          </dd>
        </div>
      {/each}
    </dl>
  </SectionCard>
{/if}

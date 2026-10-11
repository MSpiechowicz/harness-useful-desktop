<script lang="ts">
  import type { GraphRow } from './tree-layout';

  interface Props {
    row: GraphRow;
    /** Position in the list, so every row's gradient lines up into one continuous stroke. */
    position: number;
    lanes: number;
    selected: boolean;
    /** Unique per tree, so gradient ids never collide between trees on a page. */
    idBase: string;
  }

  let { row, position, lanes, selected, idBase }: Props = $props();

  const ROW_PX = 28;
  const LANE_PX = 16;
  const PAD_PX = 12;
  /** Length of one gradient sweep; the colors reflect so long sessions never run out of gradient. */
  const SWEEP_PX = 640;

  const width = $derived(lanes * LANE_PX + PAD_PX);
  const xOf = (lane: number): number => PAD_PX / 2 + LANE_PX * lane + LANE_PX / 2;
  const mid = ROW_PX / 2;
  const gradientId = $derived(`${idBase}-${position}`);
  const x = $derived(xOf(row.lane));
  const stroke = (active: boolean): string => (active ? `url(#${gradientId})` : 'var(--border-strong)');
</script>

<svg {width} height={ROW_PX} viewBox="0 0 {width} {ROW_PX}" class="shrink-0" aria-hidden="true" data-lane={row.lane}>
  <defs>
    <linearGradient id={gradientId} gradientUnits="userSpaceOnUse" x1="0" y1={-position * ROW_PX} x2="0" y2={-position * ROW_PX + SWEEP_PX} spreadMethod="reflect">
      <stop offset="0" style="stop-color: var(--accent-1)" />
      <stop offset="0.55" style="stop-color: var(--accent-2)" />
      <stop offset="1" style="stop-color: var(--accent-3)" />
    </linearGradient>
  </defs>

  {#each row.through as line (line.lane)}
    <line x1={xOf(line.lane)} y1="0" x2={xOf(line.lane)} y2={ROW_PX} stroke={stroke(line.active)} stroke-width="2" />
  {/each}

  {#if row.node === null}
    <!-- A folded run: the trunk turns into a dotted line, saying "there is more here" without covering it. -->
    <line x1={x} y1="0" x2={x} y2={ROW_PX} stroke={stroke(row.active)} stroke-width="2" stroke-linecap="round" stroke-dasharray="0.01 5" />
  {:else if row.from !== null}
    {#if row.from === row.lane}
      <line x1={x} y1="0" x2={x} y2={mid} stroke={stroke(row.active)} stroke-width="2" />
    {:else}
      <path d="M {xOf(row.from)} 0 C {xOf(row.from)} {mid}, {x} {mid / 2}, {x} {mid}" fill="none" stroke={stroke(row.active)} stroke-width="2" />
    {/if}
  {/if}

  {#if row.down && row.node !== null}
    <line x1={x} y1={mid} x2={x} y2={ROW_PX} stroke={stroke(row.active)} stroke-width="2" />
  {/if}

  {#if row.node !== null}
    {#if row.branchPoint}
      <circle cx={x} cy={mid} r="8" fill="none" stroke={stroke(row.active)} stroke-width="1.5" stroke-dasharray="3 2" />
    {/if}
    {#if selected}
      <!-- The selected node gets a soft halo instead of a bigger dot, so it reads at a glance on the tinted row. -->
      <circle cx={x} cy={mid} r="8" style="fill: var(--accent-2)" opacity="0.22" />
    {/if}
    <!-- No knockout outline: a ring in the panel colour shows as a dark band on the selected row's tint. -->
    <circle cx={x} cy={mid} r={selected ? 4.5 : 3.5} fill={row.active ? stroke(true) : 'var(--border-strong)'} />
  {/if}
</svg>

<script lang="ts">
  import type { Dial, LimitMode } from '$lib/dashboard/limit-dial';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import {
    BATTERY,
    BATTERY_CELL,
    BATTERY_IN,
    BATTERY_OUT,
    EMPTY,
    MIN_SOLID,
    RED,
    SEGS,
    TACH,
    THERMO,
    THERMO_CHANNEL,
    THERMO_LAYER_HALF,
    THERMO_TICKS,
    VU,
    VU_STRIP,
    VU_TICKS,
    WAFFLE,
    batteryCells,
    gaugeSize,
    needle,
    redEnd,
    segmentBlocks,
    tachArc,
    tachAt,
    tachTicks,
    thermoLayer,
    thermoY,
    vuArc,
    vuNeedle,
    waffleCells,
    type GaugeKind,
  } from './limit-gauge';

  interface Props {
    kind: GaugeKind;
    /** The provider's color: the fill is never recolored for a warning. */
    color: string;
    dial: Dial;
    mode: LimitMode;
    /** Smaller, when four or more share a row. */
    small: boolean;
  }

  let { kind, color, dial, mode, small }: Props = $props();

  // Ids in an svg are shared by the whole page: each chart needs its own for its clip path.
  const id = uid('limit-gauge');

  const size = $derived(gaugeSize(kind, small));
  const pct = $derived(`${Math.round(dial.value * 100)}%`);
  const word = $derived(t(mode === 'used' ? 'ecosystem.limits.dial.used' : 'ecosystem.limits.dial.left'));

  // The red end of the scale: its label is red too.
  const red = $derived(redEnd(mode));

  const ticks = $derived(tachTicks(mode));
  const blocks = $derived(segmentBlocks(dial, mode, color));
  const cells = $derived(waffleCells(dial, mode, color));
  const batteryBlocks = $derived(batteryCells(dial, mode, color));
  const vu = $derived(vuNeedle(dial.value));

  // The bulb is the bottom of the fill: coloured while the tube shows anything, empty at 0% in either mode.
  const filled = $derived(dial.value > MIN_SOLID);
</script>

<!-- The number and its word on a line of their own, under a drawing too short or too tall to hold them. -->
{#snippet reading()}
  <span class="flex flex-wrap items-baseline justify-center gap-x-1 gap-y-0.5 leading-none">
    <b class={cx('font-bold tracking-[-0.02em] text-fg tabular', small ? 'text-lg' : 'text-xl')}>{pct}</b>
    <span class="text-2xs font-medium text-fg-2">{word}</span>
  </span>
{/snippet}

<!-- The drawing is a picture: the limit's meter around it carries the words. -->
{#if kind === 'tach'}
  <svg viewBox="0 0 {size.box[0]} {size.box[1]}" width={size.width} height={size.height} class="h-auto max-w-full shrink-0" aria-hidden="true">
    <path d={tachArc(0, 1)} fill="none" class="stroke-surface-3" stroke-width="7" stroke-linecap="round" />
    <path d={tachArc(red[0], red[1])} fill="none" stroke={RED} stroke-opacity=".55" stroke-width="7" />
    {#if dial.ahead}<path d={tachArc(dial.ahead.from, dial.ahead.to)} fill="none" stroke={color} stroke-opacity=".35" stroke-width="7" />{/if}
    {#if dial.solid > MIN_SOLID}<path d={tachArc(0, dial.solid)} fill="none" stroke={color} stroke-width="7" stroke-linecap="round" />{/if}
    {#each ticks as tick, i (i)}
      <line x1={tick.x1} y1={tick.y1} x2={tick.x2} y2={tick.y2} class={tick.hot ? 'stroke-danger' : 'stroke-fg-3'} stroke-width={tick.major ? 1.8 : 1.1} stroke-linecap="round" />
    {/each}
    <polygon points={needle(TACH.cx, TACH.cy, tachAt(dial.value), TACH.r - 16, 7, 3)} class="fill-fg stroke-fg" stroke-width=".8" stroke-linejoin="round" />
    <circle cx={TACH.cx} cy={TACH.cy} r="6" class="fill-fg" />
    <circle cx={TACH.cx} cy={TACH.cy} r="2" class="fill-surface-1" />
    <text x={TACH.cx} y="92" text-anchor="middle" dominant-baseline="central" font-size="19" font-weight="700" class="fill-fg">{pct}</text>
    <text x={TACH.cx} y="106" text-anchor="middle" dominant-baseline="central" font-size="9" font-weight="500" class="fill-fg-2">{word}</text>
  </svg>
{:else if kind === 'segments'}
  <svg viewBox="0 0 {size.box[0]} {size.box[1]}" width={size.width} height={size.height} class="h-auto max-w-full shrink-0" aria-hidden="true">
    {#each blocks as block, i (i)}
      <path d={block.d} fill="none" stroke={block.fill} stroke-opacity={block.opacity} stroke-width="11" />
    {/each}
    <text x={SEGS.cx - SEGS.r} y={SEGS.cy + 14} text-anchor="middle" font-size="8.5" font-weight="700" class={mode === 'used' ? 'fill-fg-2' : 'fill-danger'}>0%</text>
    <text x={SEGS.cx + SEGS.r} y={SEGS.cy + 14} text-anchor="middle" font-size="8.5" font-weight="700" class={mode === 'used' ? 'fill-danger' : 'fill-fg-2'}>100%</text>
    <text x={SEGS.cx} y="48" text-anchor="middle" dominant-baseline="central" font-size="22" font-weight="700" letter-spacing="-.5" class="fill-fg">{pct}</text>
    <text x={SEGS.cx} y="64" text-anchor="middle" dominant-baseline="central" font-size="9" font-weight="500" class="fill-fg-2">{word}</text>
  </svg>
{:else if kind === 'vu'}
  <!-- A short face, so the number gets a line of its own under it. -->
  <div class="flex shrink-0 flex-col items-center gap-1.5">
    <svg viewBox="0 0 {size.box[0]} {size.box[1]}" width={size.width} height={size.height} class="h-auto max-w-full shrink-0" aria-hidden="true">
      <defs><clipPath id="{id}-face"><rect x="2" y="2" width="116" height="66" rx="9" /></clipPath></defs>
      <rect x="2" y="2" width="116" height="66" rx="9" class="fill-surface-2 stroke-line-strong" />
      <g clip-path="url(#{id}-face)">
        <path d={vuArc(0, 1)} fill="none" stroke={EMPTY} stroke-width="5" />
        <path d={vuArc(red[0], red[1])} fill="none" stroke={RED} stroke-opacity=".7" stroke-width="5" />
        {#if dial.ahead}<path d={vuArc(dial.ahead.from, dial.ahead.to)} fill="none" stroke={color} stroke-opacity=".35" stroke-width="5" />{/if}
        {#if dial.solid > MIN_SOLID}<path d={vuArc(0, dial.solid)} fill="none" stroke={color} stroke-width="5" />{/if}
        {#each VU_TICKS as tick, i (i)}
          <line x1={tick.x1} y1={tick.y1} x2={tick.x2} y2={tick.y2} class="stroke-fg-3" stroke-width={tick.major ? 1.6 : 1} stroke-linecap="round" />
        {/each}
        <text x="7" y="13" text-anchor="start" font-size="8.5" font-weight="700" class={mode === 'used' ? 'fill-fg-2' : 'fill-danger'}>0%</text>
        <text x="113" y="13" text-anchor="end" font-size="8.5" font-weight="700" class={mode === 'used' ? 'fill-danger' : 'fill-fg-2'}>100%</text>
        <line x1={vu.foot.x} y1={vu.foot.y} x2={vu.tip.x} y2={vu.tip.y} class="stroke-fg" stroke-width="1.8" stroke-linecap="round" />
        <rect x="2" y={VU_STRIP} width="116" height="14" class="fill-surface-3" />
      </g>
    </svg>
    {@render reading()}
  </div>
{:else if kind === 'waffle'}
  <!-- A tall grid, so the number gets a line of its own under it. -->
  <div class="flex shrink-0 flex-col items-center gap-1.5">
    <svg viewBox="0 0 {size.box[0]} {size.box[1]}" width={size.width} height={size.height} class="h-auto max-w-full shrink-0" aria-hidden="true">
      {#each cells as cell, i (i)}
        <rect x={cell.x} y={cell.y} width={WAFFLE.cell} height={WAFFLE.cell} rx="1.6" fill={cell.fill} fill-opacity={cell.opacity} />
      {/each}
    </svg>
    {@render reading()}
  </div>
{:else if kind === 'battery'}
  <div class="flex shrink-0 flex-col items-center gap-1.5">
    <svg viewBox="0 0 {size.box[0]} {size.box[1]}" width={size.width} height={size.height} class="h-auto max-w-full shrink-0" aria-hidden="true">
      <rect x="1" y="1" width={BATTERY.body} height={BATTERY.height - 2} rx="6" fill="none" class="stroke-line-strong" stroke-width={BATTERY.stroke} />
      <path d="M{BATTERY_OUT} {BATTERY.height / 2 - BATTERY.capHeight / 2} h{BATTERY.cap - 2} a2 2 0 0 1 2 2 v{BATTERY.capHeight - 4} a2 2 0 0 1 -2 2 h{2 - BATTERY.cap} z" class="fill-line-strong" />
      {#each batteryBlocks as cell, i (i)}
        <rect x={cell.x} y={BATTERY_IN} width={BATTERY_CELL} height={BATTERY.height - 2 * BATTERY_IN} rx="2" fill={cell.fill} fill-opacity={cell.opacity} />
      {/each}
    </svg>
    {@render reading()}
  </div>
{:else}
  <svg viewBox="0 0 {size.box[0]} {size.box[1]}" width={size.width} height={size.height} class="h-auto max-w-full shrink-0" aria-hidden="true">
    <defs>
      <clipPath id="{id}-glass">
        <rect x={THERMO.x - THERMO_CHANNEL / 2} y={THERMO.top} width={THERMO_CHANNEL} height={THERMO.bulb - THERMO.top} rx={THERMO_CHANNEL / 2} />
        <circle cx={THERMO.x} cy={THERMO.bulb} r="8.5" />
      </clipPath>
    </defs>
    <rect x={THERMO.x - THERMO.tube / 2} y={THERMO.top - THERMO.rim} width={THERMO.tube} height={THERMO.bulb - THERMO.top + THERMO.rim} rx={THERMO.tube / 2} class="fill-surface-3" />
    <circle cx={THERMO.x} cy={THERMO.bulb} r="11" class="fill-surface-3" />
    <g clip-path="url(#{id}-glass)">
      <rect x={THERMO.x - THERMO_LAYER_HALF} y="0" width={2 * THERMO_LAYER_HALF} height="100" fill={EMPTY} />
      {#if filled}<rect x={THERMO.x - THERMO_LAYER_HALF} y={THERMO.bottom} width={2 * THERMO_LAYER_HALF} height={THERMO.bulb + 10 - THERMO.bottom} fill={color} />{/if}
      <rect x={THERMO.x - THERMO_LAYER_HALF} width={2 * THERMO_LAYER_HALF} {...thermoLayer(red[0], red[1])} fill={RED} fill-opacity=".5" />
      {#if dial.ahead}<rect x={THERMO.x - THERMO_LAYER_HALF} width={2 * THERMO_LAYER_HALF} {...thermoLayer(dial.ahead.from, dial.ahead.to)} fill={color} fill-opacity=".35" />{/if}
      {#if dial.solid > MIN_SOLID}<rect x={THERMO.x - THERMO_LAYER_HALF} width={2 * THERMO_LAYER_HALF} {...thermoLayer(0, dial.solid, true)} fill={color} />{/if}
    </g>
    {#each THERMO_TICKS as tick, i (i)}
      <line x1={THERMO.x + THERMO.tube / 2 + 3} y1={tick.y} x2={THERMO.x + THERMO.tube / 2 + (tick.long ? 9 : 7)} y2={tick.y} class="stroke-fg-3" stroke-width="1.4" stroke-linecap="round" />
    {/each}
    <text x={THERMO.x + THERMO.tube / 2 + 12} y={thermoY(1) + 3} font-size="8.5" font-weight="700" class={mode === 'used' ? 'fill-danger' : 'fill-fg-2'}>100%</text>
    <text x={THERMO.x + THERMO.tube / 2 + 12} y={thermoY(0) + 3} font-size="8.5" font-weight="700" class={mode === 'used' ? 'fill-fg-2' : 'fill-danger'}>0%</text>
    <text x="86" y="44" text-anchor="middle" dominant-baseline="central" font-size="21" font-weight="700" letter-spacing="-.5" class="fill-fg">{pct}</text>
    <text x="86" y="60" text-anchor="middle" dominant-baseline="central" font-size="9.5" font-weight="500" class="fill-fg-2">{word}</text>
  </svg>
{/if}

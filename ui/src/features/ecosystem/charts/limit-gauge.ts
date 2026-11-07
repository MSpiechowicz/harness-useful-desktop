import type { Dial, LimitMode } from '$lib/dashboard/limit-dial';
import type { LimitGaugePref } from '$lib/ipc/bindings/LimitGaugePref';

/**
 * The geometry and cell states of the plan-limit charts, ported from the Harness Dashboard's LimitGauge.svelte and
 * LimitBar.svelte. The numbers are drawing coordinates inside each chart's own viewBox, not layout sizes.
 */
export type GaugeKind = Exclude<LimitGaugePref, 'bars'>;

/** The red end of the scale, as fractions from the 0% end: the last tenth when counting up, the first when counting down. */
export function redEnd(mode: LimitMode): [number, number] {
  return mode === 'used' ? [0.9, 1] : [0, 0.1];
}

export const RED = 'var(--danger)';
export const EMPTY = 'var(--surface-4)';
export const CELL_OFF = 'var(--surface-3)';

/** Under this a solid stretch is not drawn at all. */
export const MIN_SOLID = 0.003;

/** Pale forecast cells are the provider's color at this strength, the red end of the scale at {@link HOT_OPACITY}. */
export const PALE_OPACITY = 0.35;
export const HOT_OPACITY = 0.45;

const rad = (deg: number) => (deg * Math.PI) / 180;
const num = (n: number) => n.toFixed(2);

/** An arc of the circle round (cx, cy) between two angles in degrees, clockwise from the right, as a path. */
export function arcPath(cx: number, cy: number, r: number, from: number, to: number): string {
  const x1 = cx + r * Math.cos(rad(from));
  const y1 = cy + r * Math.sin(rad(from));
  const x2 = cx + r * Math.cos(rad(to));
  const y2 = cy + r * Math.sin(rad(to));

  return `M${num(x1)} ${num(y1)} A${r} ${r} 0 ${to - from > 180 ? 1 : 0} 1 ${num(x2)} ${num(y2)}`;
}

/** A pointer tapering from its pivot (cx, cy) along an angle, as the points of a polygon. */
export function needle(cx: number, cy: number, deg: number, length: number, back: number, wide: number): string {
  const ux = Math.cos(rad(deg));
  const uy = Math.sin(rad(deg));
  const tip = [cx + ux * length, cy + uy * length] as const;
  const tail = [cx - ux * back, cy - uy * back] as const;

  return [
    [tip[0] - uy * 0.9, tip[1] + ux * 0.9],
    [tip[0] + uy * 0.9, tip[1] - ux * 0.9],
    [tail[0] + uy * wide, tail[1] - ux * wide],
    [tail[0] - uy * wide, tail[1] + ux * wide],
  ]
    .map(([x, y]) => `${num(x!)},${num(y!)}`)
    .join(' ');
}

/** The size each chart is drawn at: its viewBox, and its width on its own and among four or more. */
const SIZES: Record<GaugeKind, { box: [number, number]; width: [number, number] }> = {
  tach: { box: [120, 112], width: [112, 92] },
  segments: { box: [120, 84], width: [120, 96] },
  vu: { box: [120, 70], width: [120, 96] },
  thermo: { box: [120, 100], width: [120, 96] },
  waffle: { box: [100, 100], width: [76, 62] },
  battery: { box: [109, 46], width: [112, 92] },
};

/** A chart's viewBox and the size it is drawn at. It never outgrows its column: the svg shrinks to fit (max-w-full, h-auto). */
export function gaugeSize(kind: GaugeKind, small: boolean): { box: [number, number]; width: number; height: number } {
  const { box, width } = SIZES[kind];
  const wide = width[small ? 1 : 0];

  return { box, width: wide, height: Math.round((wide * box[1]) / box[0]) };
}

// A chart of cells (the bars, the segmented arc, the waffle, the battery): a cell is lit when its middle is within the
// solid part, pale when within the forecast stretch. A first cell lights for any real amount, so a small share does not vanish.
const EPSILON = 1e-6;

export interface CellRule {
  count: number;
  /** A share this large lights the first cell whatever its middle says. */
  first: number;
  /** How far past the forecast stretch a cell's middle may be and still be pale. */
  slack: number;
}

export const CELL_RULES = {
  bars: { count: 20, first: 0.02, slack: 0.5 / 20 },
  segments: { count: 10, first: 0.02, slack: 0.05 },
  waffle: { count: 100, first: 0.01, slack: 0.005 },
  battery: { count: 10, first: 0.02, slack: EPSILON },
} as const satisfies Record<string, CellRule>;

export type CellLevel = 'lit' | 'pale' | 'off';

/** Whether cell `i` is lit by the solid part, pale in the forecast stretch, or off. */
export function cellLevel(i: number, rule: CellRule, solid: number, ahead: Dial['ahead']): CellLevel {
  const mid = (i + 0.5) / rule.count;

  if (mid <= solid + EPSILON || (i === 0 && solid >= rule.first)) {
    return 'lit';
  }
  if (ahead && ((mid >= ahead.from && mid <= ahead.to + rule.slack) || (i === 0 && ahead.to >= rule.first))) {
    return 'pale';
  }

  return 'off';
}

export interface Paint {
  fill: string;
  opacity: number;
}

/** The paint of a cell: the provider's color, never recolored for a warning, its pale forecast, the red end of the scale, or empty. */
export function paintOf(level: CellLevel, hot: boolean, color: string): Paint {
  if (level === 'lit') {
    return { fill: color, opacity: 1 };
  }
  if (level === 'pale') {
    return { fill: color, opacity: PALE_OPACITY };
  }

  return hot ? { fill: RED, opacity: HOT_OPACITY } : { fill: CELL_OFF, opacity: 1 };
}

/** The twenty blocks of the bars. A block is red when its middle is in the red end. */
export function barBlocks(dial: Dial, mode: LimitMode, color: string): Paint[] {
  const rule = CELL_RULES.bars;
  const [from, to] = redEnd(mode);

  return Array.from({ length: rule.count }, (_, i) => {
    const mid = (i + 0.5) / rule.count;

    return paintOf(cellLevel(i, rule, dial.solid, dial.ahead), mid >= from && mid <= to, color);
  });
}

// Eleven ticks, at every tenth, the ends and the middle longer.
const TENTHS = Array.from({ length: 11 }, (_, i) => i / 10);
const major = (f: number) => f === 0 || f === 0.5 || f === 1;

// Tachometer: 270 degrees from the lower left round to the lower right, the number in the gap at the bottom.
export const TACH = { cx: 60, cy: 56, r: 46, start: 135, sweep: 270 };
export const tachAt = (f: number) => TACH.start + TACH.sweep * f;
export const tachArc = (from: number, to: number) => arcPath(TACH.cx, TACH.cy, TACH.r, tachAt(from), tachAt(to));

export function tachTicks(mode: LimitMode): { x1: number; y1: number; x2: number; y2: number; hot: boolean; major: boolean }[] {
  return TENTHS.map((f) => {
    const a = rad(tachAt(f));
    const inner = TACH.r - 13;
    const outer = TACH.r - (major(f) ? 6 : 9);

    return {
      x1: TACH.cx + inner * Math.cos(a),
      y1: TACH.cy + inner * Math.sin(a),
      x2: TACH.cx + outer * Math.cos(a),
      y2: TACH.cy + outer * Math.sin(a),
      hot: mode === 'used' ? f >= 0.9 : f <= 0.1,
      major: major(f),
    };
  });
}

// Segmented arc: ten blocks round a half circle, lit one by one.
export const SEGS = { cx: 60, cy: 62, r: 46, gap: 3.2, count: CELL_RULES.segments.count };

export function segmentBlocks(dial: Dial, mode: LimitMode, color: string): (Paint & { d: string })[] {
  const step = 180 / SEGS.count;

  return Array.from({ length: SEGS.count }, (_, i) => {
    const d = arcPath(SEGS.cx, SEGS.cy, SEGS.r, 180 + step * i + SEGS.gap / 2, 180 + step * (i + 1) - SEGS.gap / 2);
    const hot = mode === 'used' ? i === SEGS.count - 1 : i === 0;

    return { d, ...paintOf(cellLevel(i, CELL_RULES.segments, dial.solid, dial.ahead), hot, color) };
  });
}

// VU meter: a face with a flat scale high up and a long needle from a pivot below it.
export const VU = { cx: 60, cy: 104, r: 84, start: 237, sweep: 66 };
export const vuAt = (f: number) => VU.start + VU.sweep * f;
export const vuArc = (from: number, to: number) => arcPath(VU.cx, VU.cy, VU.r, vuAt(from), vuAt(to));

export const VU_TICKS = TENTHS.map((f) => {
  const a = rad(vuAt(f));
  const inner = VU.r + 5;
  const outer = VU.r + (major(f) ? 11 : 8);

  return { x1: VU.cx + inner * Math.cos(a), y1: VU.cy + inner * Math.sin(a), x2: VU.cx + outer * Math.cos(a), y2: VU.cy + outer * Math.sin(a), major: major(f) };
});

/** The grey strip at the bottom of the face. */
export const VU_STRIP = 54;

/** The needle's tip, and its foot: it starts inside the grey strip, not at the pivot far below the face, where a sliver showed under the face. */
export function vuNeedle(value: number): { tip: { x: number; y: number }; foot: { x: number; y: number } } {
  const tip = { x: VU.cx + (VU.r + 9) * Math.cos(rad(vuAt(value))), y: VU.cy + (VU.r + 9) * Math.sin(rad(vuAt(value))) };
  const foot = { x: VU.cx + ((tip.x - VU.cx) * (VU.cy - VU_STRIP - 4)) / (VU.cy - tip.y), y: VU_STRIP + 4 };

  return { tip, foot };
}

// Thermometer: a tube on a bulb. Every layer is clipped to the glass's inside, a capsule that runs on into the bulb.
export const THERMO = { x: 30, tube: 12, rim: 2.5, bottom: 78, top: 5.5, bulb: 87 };
export const thermoY = (f: number) => THERMO.bottom - (THERMO.bottom - THERMO.top) * f;
export const THERMO_TICKS = [0, 0.25, 0.5, 0.75, 1].map((f) => ({ y: thermoY(f), long: f === 0.5 }));
export const THERMO_CHANNEL = THERMO.tube - 2 * THERMO.rim;
/** The layers inside the glass span the bulb's full width: the clip gives them the tube's and the bulb's round shapes. */
export const THERMO_LAYER_HALF = 11;

/** A layer of the tube from the share a to the share b. The fill goes on into the bulb. */
export function thermoLayer(a: number, b: number, intoBulb = false): { y: number; height: number } {
  const top = thermoY(b);
  const bottom = intoBulb ? THERMO.bulb + 10 : thermoY(a);

  return { y: top, height: Math.max(0, bottom - top) };
}

// Waffle: a hundred cells, one to a percent, filled from the bottom left row by row. The red tenth is the top row when
// counting up, the bottom row when counting down.
export const WAFFLE = { side: 10, pitch: 10, cell: 8.4, margin: 0.8 };

export function waffleCells(dial: Dial, mode: LimitMode, color: string): (Paint & { x: number; y: number })[] {
  return Array.from({ length: WAFFLE.side * WAFFLE.side }, (_, i) => {
    const col = i % WAFFLE.side;
    const row = Math.floor(i / WAFFLE.side);
    const hot = mode === 'used' ? i >= 90 : i < 10;

    return {
      x: WAFFLE.margin + col * WAFFLE.pitch,
      y: WAFFLE.margin + (WAFFLE.side - 1 - row) * WAFFLE.pitch,
      ...paintOf(cellLevel(i, CELL_RULES.waffle, dial.solid, dial.ahead), hot, color),
    };
  });
}

// Battery: ten cells in an outlined body with a terminal on the right.
export const BATTERY = { body: 102, height: 46, stroke: 2, inset: 3, cap: 5, capHeight: 14, count: CELL_RULES.battery.count, gap: 2.5 };

// The stroke is centred on the body rect (at x = 1), so its inside starts at 1 + stroke / 2 and its outside ends at
// body + 1 + stroke / 2, where the terminal begins. The cells keep clear of the stroke and nothing translucent overlaps.
export const BATTERY_IN = 1 + BATTERY.stroke / 2 + BATTERY.inset;
export const BATTERY_OUT = BATTERY.body + 1 + BATTERY.stroke / 2;
export const BATTERY_CELL = (BATTERY.body - BATTERY.inset - BATTERY_IN - (BATTERY.count - 1) * BATTERY.gap) / BATTERY.count;

/** What is left now, and what will be left once the forecast has run its course. The battery draws the charge whatever the mode. */
export function batteryCharge(dial: Dial, mode: LimitMode): { now: number; reset: number } {
  if (mode === 'used') {
    return { now: 1 - dial.value, reset: 1 - (dial.ahead?.to ?? dial.value) };
  }

  return { now: dial.value, reset: dial.ahead ? dial.solid : dial.value };
}

/** The ten cells: full up to what is left at the reset, pale up to what is left now. The first is red when empty. */
export function batteryCells(dial: Dial, mode: LimitMode, color: string): (Paint & { x: number })[] {
  const charge = batteryCharge(dial, mode);
  const ahead = { from: charge.reset, to: charge.now };

  return Array.from({ length: BATTERY.count }, (_, i) => ({
    x: BATTERY_IN + i * (BATTERY_CELL + BATTERY.gap),
    ...paintOf(cellLevel(i, CELL_RULES.battery, charge.reset, ahead), i === 0, color),
  }));
}

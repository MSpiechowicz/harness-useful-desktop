/**
 * Geometry of the Harness family diagram: Harness Desktop is a chip, every member a module seated in a socket and wired
 * to the chip by a right-angle trace. All numbers are px of a fixed stage the SVG maps 1:1, so every trace is a
 * horizontal or vertical line on whole pixels. Components read the result and never compute a position themselves.
 */

export type Point = readonly [number, number];

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Side = 'left' | 'right';

export interface ModuleSlot extends Rect {
  index: number;
  side: Side;
  /** From the module's inner edge to the chip (wide: its pin, narrow: the trunk). */
  path: Point[];
  /** The route a data blip travels: the path itself, plus the trunk in the narrow layout. */
  route: Point[];
}

export interface KernelLayout {
  narrow: boolean;
  width: number;
  height: number;
  chip: Rect;
  status: Rect;
  modules: ModuleSlot[];
  /** Narrow only: the spine the stacked modules hang from. */
  trunk: Point[] | null;
  /** Pin centres along each side edge of the chip, as a y offset from the chip's top. */
  pins: Record<Side, number[]>;
}

export const MODULE_H = 44;
export const STATUS_H = 16;

const WIDE = { width: 432, chip: 100, moduleW: 124, pitch: 58, jog: 21, statusGap: 12, statusPad: 2 } as const;
const NARROW = { width: 292, chip: 84, moduleW: 138, pitch: 54, chipTop: 4, chipGap: 4, firstRow: 36, statusGap: 10, statusPad: 6 } as const;

/**
 * The chip pins face the modules at these fractions of its height (a lone pin sits in the middle). A column of four needs
 * the wider span for its traces to jog to their pins without crossing.
 */
const PIN_SPAN = { few: { from: 0.2, to: 0.8 }, many: { from: 0.1, to: 0.9 } } as const;
const FEW_PINS = 3;

/** One wide column holds four modules: beyond eight members the chip is too short for every trace to reach its own pin, so the stacked layout takes over. */
export const MAX_WIDE_MODULES = 8;

/** The width of the wide stage: with less room than this the stacked one takes over. */
export const WIDE_STAGE_PX = WIDE.width;

/** The stacked layout has two columns of modules under the chip. */
const COLUMNS = 2;

function pinOffsets(count: number, chip: number): number[] {
  if (count === 0) {
    return [];
  }

  if (count === 1) {
    return [chip / 2];
  }

  const { from, to } = count <= FEW_PINS ? PIN_SPAN.few : PIN_SPAN.many;

  return Array.from({ length: count }, (_, row) => chip * (from + ((to - from) * row) / (count - 1)));
}

/** A route without the zero-length hops a straight run would otherwise carry. */
function simplify(points: Point[]): Point[] {
  return points.filter((point, index) => index === 0 || point[0] !== points[index - 1][0] || point[1] !== points[index - 1][1]);
}

/** Wide: modules in one column on each side, the first half on the left, every trace jogging to its own pin. */
function layoutWide(count: number, statusWidth: number): KernelLayout {
  const { width, chip: chipSize, moduleW, pitch, jog, statusGap, statusPad } = WIDE;
  const left = Math.ceil(count / 2);
  const rows = left;
  const blockH = rows === 0 ? 0 : (rows - 1) * pitch + MODULE_H;
  const contentH = Math.max(chipSize, blockH);
  const blockTop = (contentH - blockH) / 2;
  const cx = width / 2;
  const cy = contentH / 2;
  const chip: Rect = { x: cx - chipSize / 2, y: cy - chipSize / 2, w: chipSize, h: chipSize };
  const pins: Record<Side, number[]> = { left: pinOffsets(left, chipSize), right: pinOffsets(count - left, chipSize) };
  const modules: ModuleSlot[] = [];

  for (let index = 0; index < count; index++) {
    const side: Side = index < left ? 'left' : 'right';
    const row = side === 'left' ? index : index - left;
    const inColumn = side === 'left' ? left : count - left;
    // A shorter column is centred against the taller one.
    const y = blockTop + ((rows - inColumn) * pitch) / 2 + row * pitch;
    const x = side === 'left' ? 0 : width - moduleW;
    const centre = y + MODULE_H / 2;
    const pinY = chip.y + pins[side][row];
    const edge = side === 'left' ? moduleW : x;
    const bend = side === 'left' ? moduleW + jog : x - jog;
    const pin = side === 'left' ? chip.x : chip.x + chipSize;
    const path = simplify([[edge, centre], [bend, centre], [bend, pinY], [pin, pinY]]);

    modules.push({ index, x, y, w: moduleW, h: MODULE_H, side, path, route: path });
  }

  return {
    narrow: false,
    width,
    height: contentH + statusGap + STATUS_H + statusPad,
    chip,
    status: { x: cx - statusWidth / 2, y: contentH + statusGap, w: statusWidth, h: STATUS_H },
    modules,
    trunk: null,
    pins,
  };
}

/** Narrow: the chip on top, the modules stacked in two columns under it and every trace running to one spine. */
function layoutNarrow(count: number, statusWidth: number): KernelLayout {
  const { width, chip: chipSize, moduleW, pitch, chipTop, chipGap, firstRow, statusGap, statusPad } = NARROW;
  const cx = width / 2;
  const chip: Rect = { x: cx - chipSize / 2, y: chipTop, w: chipSize, h: chipSize };
  const spineTop = chipTop + chipSize + chipGap;
  const rows = Math.ceil(count / COLUMNS);
  const lastCentre = spineTop + firstRow + Math.max(0, rows - 1) * pitch;
  const modules: ModuleSlot[] = [];

  for (let index = 0; index < count; index++) {
    const side: Side = index % COLUMNS === 0 ? 'left' : 'right';
    const centre = spineTop + firstRow + Math.floor(index / COLUMNS) * pitch;
    const x = side === 'left' ? 0 : width - moduleW;
    const edge = side === 'left' ? moduleW : x;
    const path: Point[] = [[edge, centre], [cx, centre]];

    modules.push({ index, x, y: centre - MODULE_H / 2, w: moduleW, h: MODULE_H, side, path, route: [...path, [cx, spineTop]] });
  }

  const bottom = rows === 0 ? spineTop : lastCentre + MODULE_H / 2;
  const statusTop = bottom + statusGap;

  return {
    narrow: true,
    width,
    height: statusTop + STATUS_H + statusPad,
    chip,
    status: { x: cx - statusWidth / 2, y: statusTop, w: statusWidth, h: STATUS_H },
    modules,
    trunk: rows === 0 ? null : [[cx, spineTop], [cx, lastCentre]],
    // The narrow chip keeps its three decorative pins per side.
    pins: { left: pinOffsets(3, chipSize), right: pinOffsets(3, chipSize) },
  };
}

/** The layout of `count` modules; `statusWidth` is the width of the status line it must keep clear. */
export function layoutKernel(count: number, narrow: boolean, statusWidth = 0): KernelLayout {
  return narrow || count > MAX_WIDE_MODULES ? layoutNarrow(count, statusWidth) : layoutWide(count, statusWidth);
}

/** `M x y L x y ...` for an SVG path. */
export function pathData(points: readonly Point[]): string {
  return points.map(([x, y], index) => `${index === 0 ? 'M' : 'L'} ${x} ${y}`).join(' ');
}

/** The point a fraction `u` (0 to 1) of the way along a route. */
export function pointAlong(route: readonly Point[], u: number): Point {
  const lengths = route.slice(1).map(([x, y], index) => Math.hypot(x - route[index][0], y - route[index][1]));
  const total = lengths.reduce((sum, length) => sum + length, 0);
  let remaining = Math.min(1, Math.max(0, u)) * total;

  for (let index = 0; index < lengths.length; index++) {
    if (remaining <= lengths[index] || index === lengths.length - 1) {
      const k = lengths[index] === 0 ? 0 : Math.min(1, remaining / lengths[index]);
      const [x0, y0] = route[index];
      const [x1, y1] = route[index + 1];

      return [x0 + (x1 - x0) * k, y0 + (y1 - y0) * k];
    }

    remaining -= lengths[index];
  }

  return route[route.length - 1];
}

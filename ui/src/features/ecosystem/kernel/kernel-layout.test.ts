import { describe, expect, it } from 'vitest';

import { layoutKernel, MAX_WIDE_MODULES, pathData, pointAlong, type KernelLayout, type Point, type Rect } from './kernel-layout';

// The status line of the longest state: "Harness Knowledge · Update available" is about this wide.
const STATUS_WIDTH = 200;
const CLEARANCE = 6;

function gap(a: Rect, b: Rect): number {
  const dx = Math.max(a.x - (b.x + b.w), b.x - (a.x + a.w), 0);
  const dy = Math.max(a.y - (b.y + b.h), b.y - (a.y + a.h), 0);

  return Math.hypot(dx, dy);
}

/** Points along a route, a pixel apart. */
function samples(route: readonly Point[]): Point[] {
  const points: Point[] = [];

  for (let index = 1; index < route.length; index++) {
    const [x0, y0] = route[index - 1];
    const [x1, y1] = route[index];
    const steps = Math.max(1, Math.ceil(Math.hypot(x1 - x0, y1 - y0)));

    for (let step = 0; step <= steps; step++) {
      points.push([x0 + ((x1 - x0) * step) / steps, y0 + ((y1 - y0) * step) / steps]);
    }
  }

  return points;
}

function distance(points: Point[], rect: Rect): number {
  return Math.min(...points.map(([x, y]) => gap({ x, y, w: 0, h: 0 }, rect)));
}

function between(a: Point[], b: Point[]): number {
  return Math.min(...a.flatMap(([x0, y0]) => b.map(([x1, y1]) => Math.hypot(x0 - x1, y0 - y1))));
}

function sameRow(layout: KernelLayout, a: number, b: number): boolean {
  return layout.narrow && layout.modules[a].y === layout.modules[b].y;
}

/** Every rule of the drawing a person would see as a collision. */
function collisions(layout: KernelLayout): string[] {
  const found: string[] = [];
  const stage: Rect = { x: 0, y: 0, w: layout.width, h: layout.height };
  const wires = layout.modules.map((slot) => samples(slot.path));
  const spine = layout.trunk ? samples(layout.trunk) : null;

  const inside = (name: string, rect: Rect) => {
    if (rect.x < 0 || rect.y < 0 || rect.x + rect.w > stage.w || rect.y + rect.h > stage.h) {
      found.push(`${name} leaves the stage`);
    }
  };

  inside('chip', layout.chip);
  inside('status', layout.status);

  if (gap(layout.chip, layout.status) < CLEARANCE) {
    found.push('chip touches the status line');
  }

  layout.modules.forEach((slot, index) => {
    inside(`module ${index}`, slot);

    if (gap(slot, layout.chip) < CLEARANCE) {
      found.push(`module ${index} touches the chip`);
    }

    if (gap(slot, layout.status) < CLEARANCE) {
      found.push(`module ${index} touches the status line`);
    }

    if (distance(wires[index], layout.status) < CLEARANCE) {
      found.push(`trace ${index} touches the status line`);
    }

    layout.modules.forEach((other, otherIndex) => {
      if (otherIndex <= index) {
        return;
      }

      if (gap(slot, other) < CLEARANCE) {
        found.push(`modules ${index} and ${otherIndex} touch`);
      }

      if (!sameRow(layout, index, otherIndex) && between(wires[index], wires[otherIndex]) < CLEARANCE) {
        found.push(`traces ${index} and ${otherIndex} touch`);
      }
    });

    layout.modules.forEach((other, otherIndex) => {
      if (otherIndex !== index && distance(wires[index], other) < CLEARANCE) {
        found.push(`trace ${index} runs into module ${otherIndex}`);
      }
    });

    if (spine && distance(spine, layout.status) < CLEARANCE) {
      found.push('the spine touches the status line');
    }
  });

  return found;
}

const COUNTS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 12];

describe('Kernel layout', () => {
  it.each(COUNTS.flatMap((count) => [true, false].map((narrow) => [count, narrow] as const)))('keeps %i modules (narrow %s) clear of each other, the chip, the status line and the edges', (count, narrow) => {
    expect(collisions(layoutKernel(count, narrow, STATUS_WIDTH))).toEqual([]);
  });

  it('puts the first half of the members on the left and the rest on the right of the chip', () => {
    const layout = layoutKernel(6, false);

    expect(layout.modules.map((slot) => slot.side)).toEqual(['left', 'left', 'left', 'right', 'right', 'right']);
    expect(layout.modules.every((slot) => (slot.side === 'left' ? slot.x + slot.w <= layout.chip.x : slot.x >= layout.chip.x + layout.chip.w))).toBe(true);
  });

  it('wires every module to its own pin on the facing edge of the chip', () => {
    const layout = layoutKernel(6, false);

    for (const slot of layout.modules) {
      const [pinX, pinY] = slot.path[slot.path.length - 1];

      expect(pinX).toBe(slot.side === 'left' ? layout.chip.x : layout.chip.x + layout.chip.w);
      expect(pinY).toBeGreaterThan(layout.chip.y);
      expect(pinY).toBeLessThan(layout.chip.y + layout.chip.h);
    }

    const pins = layout.modules.filter((slot) => slot.side === 'left').map((slot) => slot.path[slot.path.length - 1][1]);

    expect(new Set(pins).size).toBe(3);
  });

  it('draws only horizontal and vertical traces, on whole pixels', () => {
    for (const narrow of [false, true]) {
      for (const slot of layoutKernel(6, narrow).modules) {
        slot.path.forEach(([x, y], index) => {
          expect(Number.isInteger(x) && Number.isInteger(y)).toBe(true);

          if (index > 0) {
            const [px, py] = slot.path[index - 1];

            expect(px === x || py === y).toBe(true);
          }
        });
      }
    }
  });

  it('stacks the modules in two columns under the chip when narrow, joined by one spine', () => {
    const layout = layoutKernel(6, true);

    expect(layout.chip.y + layout.chip.h).toBeLessThan(Math.min(...layout.modules.map((slot) => slot.y)));
    expect(new Set(layout.modules.map((slot) => slot.x)).size).toBe(2);
    expect(layout.modules.map((slot) => slot.side)).toEqual(['left', 'right', 'left', 'right', 'left', 'right']);
    expect(layout.trunk).not.toBeNull();
    expect(layout.width).toBeLessThan(layoutKernel(6, false).width);
  });

  it('stacks a family too big for the wide stage', () => {
    expect(layoutKernel(MAX_WIDE_MODULES, false).narrow).toBe(false);
    expect(layoutKernel(MAX_WIDE_MODULES + 1, false).narrow).toBe(true);
  });

  it('centres a shorter column against the taller one', () => {
    const layout = layoutKernel(5, false);
    const left = layout.modules.filter((slot) => slot.side === 'left');
    const right = layout.modules.filter((slot) => slot.side === 'right');
    const middle = (slots: typeof left) => (slots[0].y + slots[slots.length - 1].y + slots[0].h) / 2;

    expect(middle(right)).toBe(middle(left));
  });

  it('describes a route as path data and finds the point a fraction along it', () => {
    const route: Point[] = [[0, 0], [10, 0], [10, 10]];

    expect(pathData(route)).toBe('M 0 0 L 10 0 L 10 10');
    expect(pointAlong(route, 0)).toEqual([0, 0]);
    expect(pointAlong(route, 0.5)).toEqual([10, 0]);
    expect(pointAlong(route, 0.75)).toEqual([10, 5]);
    expect(pointAlong(route, 1)).toEqual([10, 10]);
  });
});

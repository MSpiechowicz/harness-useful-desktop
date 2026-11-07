import { describe, expect, it } from 'vitest';

import type { Dial } from '$lib/dashboard/limit-dial';

import { CELL_RULES, arcPath, barBlocks, batteryCells, batteryCharge, cellLevel, gaugeSize, redEnd, waffleCells } from './limit-gauge';

const COLOR = '#de602c';
const plain = (value: number): Dial => ({ value, solid: value, ahead: null });

/** One letter a cell: S solid, p pale forecast, R red end, . empty. */
const letters = (paints: { fill: string; opacity: number }[]) => paints.map((paint) => (paint.fill === COLOR ? (paint.opacity === 1 ? 'S' : 'p') : paint.opacity === 1 ? '.' : 'R')).join('');

describe('cellLevel', () => {
  it('lights a cell when its middle is within the solid part, and the first for any real amount', () => {
    const rule = CELL_RULES.bars;

    expect(cellLevel(8, rule, 0.45, null)).toBe('lit');
    expect(cellLevel(9, rule, 0.45, null)).toBe('off');
    expect(cellLevel(0, rule, 0.02, null)).toBe('lit');
    expect(cellLevel(0, rule, 0.01, null)).toBe('off');
  });

  it('makes a cell in the forecast stretch pale, up to half a cell past it', () => {
    const rule = CELL_RULES.bars;
    const ahead = { from: 0.4, to: 0.56 };

    expect(cellLevel(7, rule, 0.4, ahead)).toBe('lit');
    expect(cellLevel(8, rule, 0.4, ahead)).toBe('pale');
    expect(cellLevel(11, rule, 0.4, ahead)).toBe('pale');
    expect(cellLevel(12, rule, 0.4, ahead)).toBe('off');
  });
});

describe('bars', () => {
  it('light twenty blocks from the 0% end and keep the red end for the last tenth when counting up', () => {
    expect(letters(barBlocks(plain(0.43), 'used', COLOR))).toBe('SSSSSSSSS' + '.........' + 'RR');
  });

  it('keep the red end at the first tenth when counting down, and never recolor what is lit', () => {
    expect(redEnd('left')).toEqual([0, 0.1]);
    expect(letters(barBlocks(plain(0.05), 'left', COLOR))).toBe('S' + 'R' + '.'.repeat(18));
    expect(letters(barBlocks(plain(0.99), 'used', COLOR))).toBe('S'.repeat(20));
  });

  it('draw the forecast stretch pale', () => {
    expect(letters(barBlocks({ value: 0.4, solid: 0.4, ahead: { from: 0.4, to: 0.56 } }, 'used', COLOR))).toBe('S'.repeat(8) + 'p'.repeat(4) + '.'.repeat(6) + 'RR');
  });
});

describe('waffle', () => {
  it('fills a hundred cells from the bottom left, with the red tenth on the top row when counting up and the bottom row when counting down', () => {
    const red = (cells: { fill: string; opacity: number; y: number }[]) => new Set(cells.filter((cell) => cell.fill !== COLOR && cell.opacity < 1).map((cell) => cell.y));
    const used = waffleCells(plain(0.5), 'used', COLOR);

    expect(used.filter((cell) => cell.fill === COLOR)).toHaveLength(50);
    expect(red(used)).toEqual(new Set([0.8]));
    expect(red(waffleCells(plain(0), 'left', COLOR))).toEqual(new Set([90.8]));
  });
});

describe('battery', () => {
  it('always draws what is left: full at the reset, pale up to now', () => {
    const forecast: Dial = { value: 0.43, solid: 0.43, ahead: { from: 0.43, to: 0.8 } };

    expect(batteryCharge(forecast, 'used')).toEqual({ now: expect.closeTo(0.57), reset: expect.closeTo(0.2) });
    expect(letters(batteryCells(forecast, 'used', COLOR))).toBe('SSpppp....');

    // Counting down, the dial already holds what is left: the same charge.
    const counted: Dial = { value: 0.57, solid: 0.2, ahead: { from: 0.2, to: 0.57 } };

    expect(letters(batteryCells(counted, 'left', COLOR))).toBe('SSpppp....');
  });

  it('shows an empty battery with its first cell red', () => {
    expect(letters(batteryCells(plain(1), 'used', COLOR))).toBe('R.........');
  });
});

describe('geometry', () => {
  it('sizes a chart for a lone limit and a smaller one among four or more', () => {
    expect(gaugeSize('tach', false)).toMatchObject({ width: 112, height: 105 });
    expect(gaugeSize('tach', true)).toMatchObject({ width: 92, height: 86 });
  });

  it('flags an arc of more than half a circle as large', () => {
    expect(arcPath(0, 0, 10, 0, 90)).toBe('M10.00 0.00 A10 10 0 0 1 0.00 10.00');
    expect(arcPath(0, 0, 10, 0, 270)).toMatch(/A10 10 0 1 1 /);
  });
});

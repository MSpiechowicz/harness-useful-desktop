/** Squares in a waffle: one per percent. */
export const WAFFLE_CELLS = 100;

/**
 * How many of the 100 squares each value gets: largest remainder, so the squares always add up to 100, and any
 * nonzero value keeps at least one square so a small cause never vanishes from the chart.
 */
export function waffleCells(values: number[]): number[] {
  const total = values.reduce((sum, value) => sum + value, 0);

  if (total <= 0) {
    return values.map(() => 0);
  }

  const exact = values.map((value) => (value / total) * WAFFLE_CELLS);
  const cells = exact.map((value, index) => (values[index] > 0 ? Math.max(1, Math.floor(value)) : 0));
  let left = WAFFLE_CELLS - cells.reduce((sum, count) => sum + count, 0);

  // Hand out the remaining squares by the largest fractional part; take back any extra from the largest value.
  const byRemainder = exact.map((value, index) => ({ index, rest: value - Math.floor(value) })).sort((a, b) => b.rest - a.rest);

  for (const { index } of byRemainder) {
    if (left <= 0) {
      break;
    }

    if (values[index] > 0) {
      cells[index] += 1;
      left -= 1;
    }
  }

  while (left < 0) {
    const largest = cells.indexOf(Math.max(...cells));
    cells[largest] -= 1;
    left += 1;
  }

  return cells;
}

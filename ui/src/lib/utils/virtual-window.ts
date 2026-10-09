/**
 * Windowing math for VirtualList, kept free of the DOM so it can be tested directly.
 * `offsets[i]` is the top of item `i`; `offsets[count]` is the total height.
 */

export function computeOffsets(count: number, heightOf: (index: number) => number): number[] {
  const offsets = new Array<number>(count + 1);
  offsets[0] = 0;

  for (let index = 0; index < count; index += 1) {
    offsets[index + 1] = offsets[index] + heightOf(index);
  }

  return offsets;
}

/** Index of the item containing `position` (binary search over offsets). */
export function indexAt(offsets: number[], position: number): number {
  const count = offsets.length - 1;
  let low = 0;
  let high = count - 1;

  while (low < high) {
    const middle = (low + high + 1) >> 1;

    if (offsets[middle] <= position) {
      low = middle;
    } else {
      high = middle - 1;
    }
  }

  return Math.max(0, low);
}

export interface VirtualWindow {
  start: number;
  /** Exclusive. */
  end: number;
  total: number;
}

export function computeWindow(offsets: number[], scrollTop: number, viewport: number, overscan: number): VirtualWindow {
  const count = offsets.length - 1;
  const total = offsets[count];

  if (count === 0) {
    return { start: 0, end: 0, total: 0 };
  }

  const first = indexAt(offsets, Math.max(0, scrollTop));
  const last = indexAt(offsets, Math.max(0, scrollTop + viewport));

  return {
    start: Math.max(0, first - overscan),
    end: Math.min(count, last + 1 + overscan),
    total,
  };
}

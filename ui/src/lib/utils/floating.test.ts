import { describe, expect, it } from 'vitest';

import { floatingPosition } from './floating';

const viewport = { width: 1000, height: 600 };
const size = { width: 120, height: 28 };

describe('floatingPosition', () => {
  it('centers above the anchor when there is room', () => {
    const anchor = { left: 400, top: 300, width: 40, height: 32 };

    expect(floatingPosition(anchor, size, 'top', viewport)).toEqual({ side: 'top', left: 360, top: 264 });
  });

  it('flips to the opposite side when the preferred one leaves the viewport', () => {
    const anchor = { left: 400, top: 10, width: 40, height: 32 };

    const result = floatingPosition(anchor, size, 'top', viewport);

    expect(result.side).toBe('bottom');
    expect(result.top).toBe(50);
  });

  it('shifts a label back inside the window edge instead of clipping it', () => {
    const anchor = { left: 960, top: 300, width: 32, height: 32 };

    const result = floatingPosition(anchor, size, 'bottom', viewport);

    expect(result.left + size.width).toBeLessThanOrEqual(viewport.width - 8);
    expect(result.left).toBeGreaterThanOrEqual(8);
  });

  it('opens to the left of an anchor at the right window edge', () => {
    const anchor = { left: 960, top: 300, width: 32, height: 32 };

    expect(floatingPosition(anchor, size, 'right', viewport).side).toBe('left');
  });
});

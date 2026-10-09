export type FloatingSide = 'top' | 'bottom' | 'left' | 'right';

interface Size {
  width: number;
  height: number;
}

interface Rect extends Size {
  left: number;
  top: number;
}

export interface FloatingPosition {
  left: number;
  top: number;
  side: FloatingSide;
}

const GAP: Record<FloatingSide, number> = { top: 8, bottom: 8, left: 12, right: 12 };
const OPPOSITE: Record<FloatingSide, FloatingSide> = { top: 'bottom', bottom: 'top', left: 'right', right: 'left' };
const VIEWPORT_MARGIN = 8;

function place(anchor: Rect, size: Size, side: FloatingSide): { left: number; top: number } {
  const gap = GAP[side];
  const centeredLeft = anchor.left + (anchor.width - size.width) / 2;
  const centeredTop = anchor.top + (anchor.height - size.height) / 2;

  switch (side) {
    case 'top':
      return { left: centeredLeft, top: anchor.top - size.height - gap };
    case 'bottom':
      return { left: centeredLeft, top: anchor.top + anchor.height + gap };
    case 'left':
      return { left: anchor.left - size.width - gap, top: centeredTop };
    case 'right':
      return { left: anchor.left + anchor.width + gap, top: centeredTop };
  }
}

function fits(position: { left: number; top: number }, size: Size, viewport: Size): boolean {
  return (
    position.left >= VIEWPORT_MARGIN &&
    position.top >= VIEWPORT_MARGIN &&
    position.left + size.width <= viewport.width - VIEWPORT_MARGIN &&
    position.top + size.height <= viewport.height - VIEWPORT_MARGIN
  );
}

function clamp(value: number, size: number, viewport: number): number {
  return Math.max(VIEWPORT_MARGIN, Math.min(value, viewport - size - VIEWPORT_MARGIN));
}

/**
 * Viewport coordinates for a floating label next to `anchor`: the requested side, flipped to the
 * opposite one when it would leave the viewport, then shifted so it never sticks out of a window edge.
 */
export function floatingPosition(anchor: Rect, size: Size, side: FloatingSide, viewport: Size): FloatingPosition {
  const preferred = place(anchor, size, side);
  const flippedSide = OPPOSITE[side];
  const flipped = place(anchor, size, flippedSide);

  const useFlipped = !fits(preferred, size, viewport) && fits(flipped, size, viewport);
  const chosenSide = useFlipped ? flippedSide : side;
  const chosen = useFlipped ? flipped : preferred;

  return {
    side: chosenSide,
    left: clamp(chosen.left, size.width, viewport.width),
    top: clamp(chosen.top, size.height, viewport.height),
  };
}

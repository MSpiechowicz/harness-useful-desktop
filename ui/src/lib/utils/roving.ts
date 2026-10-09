export type Orientation = 'horizontal' | 'vertical';

/**
 * Next focus index for a roving-tabindex group, or null when the key is not
 * a navigation key. Wraps at both ends.
 */
export function rovingIndex(key: string, current: number, count: number, orientation: Orientation): number | null {
  const previousKey = orientation === 'horizontal' ? 'ArrowLeft' : 'ArrowUp';
  const nextKey = orientation === 'horizontal' ? 'ArrowRight' : 'ArrowDown';

  if (key === nextKey) {
    return (current + 1) % count;
  }
  if (key === previousKey) {
    return (current - 1 + count) % count;
  }
  if (key === 'Home') {
    return 0;
  }
  if (key === 'End') {
    return count - 1;
  }

  return null;
}

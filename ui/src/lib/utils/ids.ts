let counter = 0;

/** Stable, unique DOM id for aria wiring (`aria-labelledby`, `aria-controls`, ...). */
export function uid(prefix: string): string {
  counter += 1;

  return `${prefix}-${counter}`;
}

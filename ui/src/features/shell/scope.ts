/**
 * "12 sessions · 3 on this branch": the state line under a view's title. The harness and the project are already
 * named by the switchers in the top bar, so the header only adds what is specific to the view.
 */
export function scopeLabel(...parts: (string | null | undefined)[]): string {
  return parts.filter((part): part is string => Boolean(part)).join(' · ');
}

/** "3 sessions" / "1 session". */
export function count(amount: number, noun: string): string {
  return `${amount} ${noun}${amount === 1 ? '' : 's'}`;
}

import { scaled } from '$lib/design/motion';

/** How far a row leaves to the right, in px. */
const TRAVEL_PX = 16;

/**
 * Plays the exit of a table row (it fades and slips aside) and waits for it. Answers the way to bring the row back, for a call that failed
 * after the row was told to go. With motion off there is nothing to wait for.
 */
export async function exitRow(key: string | number): Promise<() => void> {
  const row = document.querySelector<HTMLElement>(`[data-row="${CSS.escape(String(key))}"]`);
  const duration = scaled(220);

  if (!row || duration === 0 || typeof row.animate !== 'function') {
    return () => {};
  }

  const leaving = row.animate([{ opacity: 1, transform: 'translateX(0)' }, { opacity: 0, transform: `translateX(${TRAVEL_PX}px)` }], { duration, easing: 'cubic-bezier(0.16, 1, 0.3, 1)', fill: 'forwards' });

  await leaving.finished.catch(() => {});

  return () => leaving.cancel();
}

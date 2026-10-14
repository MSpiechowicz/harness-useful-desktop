/**
 * Calls `run` every `ms` while the window is visible, and again whenever it regains focus, so an open view shows what
 * changed on disk without anyone asking. Returns the cleanup for an `$effect`.
 */
export function autoRefresh(run: () => void, ms: number): () => void {
  const tick = (): void => {
    if (!document.hidden) {
      run();
    }
  };

  const timer = setInterval(tick, ms);
  window.addEventListener('focus', tick);

  return () => {
    clearInterval(timer);
    window.removeEventListener('focus', tick);
  };
}

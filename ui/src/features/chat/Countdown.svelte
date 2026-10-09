<script lang="ts">
  interface Props {
    timeoutMs: number;
    /** Restarts the countdown when the request changes. */
    requestId: string;
    ontimeout: () => void;
  }

  let { timeoutMs, requestId, ontimeout }: Props = $props();

  const TICK_MS = 250;

  let remainingMs = $state(0);

  // The agent enforces the timeout too; the countdown only tells the user how long is left.
  $effect(() => {
    void requestId;
    const deadline = Date.now() + timeoutMs;
    remainingMs = timeoutMs;

    const timer = setInterval(() => {
      remainingMs = Math.max(0, deadline - Date.now());

      if (remainingMs === 0) {
        clearInterval(timer);
        ontimeout();
      }
    }, TICK_MS);

    return () => clearInterval(timer);
  });
</script>

<span class="tabular shrink-0 rounded-full bg-surface-3 px-2 py-0.5 text-xs text-fg-2" role="timer" aria-label="Time left to answer">{Math.ceil(remainingMs / 1000)}s</span>

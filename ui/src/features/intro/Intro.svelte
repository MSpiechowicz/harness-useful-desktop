<script lang="ts">
  import Logo from '$lib/components/Logo.svelte';
  import { motion } from '$lib/stores/motion.svelte';

  import { intro, INTRO_EXIT_MS, INTRO_LEAD_MS } from './intro.svelte';

  /** The lockup is the centrepiece of the window: the approved stage (mark 96, name 44) at 1.75 times its size. */
  const SCALE = 1.75;
  const MARK_SIZE = 96 * SCALE;
  const NAME_SIZE = 44 * SCALE;

  // Only the full-motion intro shows the logo; reduced motion is a plain crossfade of the shell.
  const visible = $derived(motion.level === 'full' && intro.active);
  const leaving = $derived(intro.phase === 'handoff');

  function skip(): void {
    if (intro.active) {
      intro.skip();
    }
  }
</script>

<!-- Any key or click skips instantly; otherwise the shell takes over as soon as the logo's launch is over. -->
<svelte:window onkeydown={skip} onpointerdown={skip} />

{#if visible}
  <!-- The intro always plays on the dark canvas, whatever the theme. -->
  <div class="theme-dark pointer-events-none fixed inset-0 z-[100] grid place-items-center" data-leaving={leaving ? '' : undefined} aria-hidden="true">
    <!-- Keeps the stage dark while the lockup leaves, then lifts to reveal the next screen, which enters as the veil goes. -->
    <div class="veil absolute inset-0 bg-canvas" style:--lead="{INTRO_LEAD_MS}ms" style:--exit="{INTRO_EXIT_MS}ms"></div>

    <div class="lockup relative" style:--exit="{INTRO_EXIT_MS}ms">
      <Logo size={MARK_SIZE} nameSize={NAME_SIZE} wordmark animate="launch" onfinish={() => intro.launched()} class="[--logo-gap:0.59em] [--logo-name-weight:600] [--logo-tracking:-0.03em]" />
    </div>
  </div>
{/if}

<style>
  /* The lockup leaves first: it fades, grows slightly and softens, so the next screen never overlaps it. */
  .lockup {
    transition:
      opacity var(--exit) var(--ease-out-expo),
      transform var(--exit) var(--ease-out-expo),
      filter var(--exit) var(--ease-out-expo);
  }

  [data-leaving] .lockup {
    opacity: 0;
    transform: scale(1.06);
    filter: blur(8px);
  }

  /* The aurora behind is already at its dim intro level, so the veil only needs to appear once the hand-over starts. */
  .veil {
    opacity: 0;
  }

  [data-leaving] .veil {
    animation: veil-lift 0.42s var(--lead) var(--ease-out-expo) both;
  }

  @keyframes veil-lift {
    from {
      opacity: 0.86;
    }
    to {
      opacity: 0;
    }
  }
</style>

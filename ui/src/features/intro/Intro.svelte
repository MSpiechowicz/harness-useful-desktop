<script lang="ts">
  import '$lib/brand/vendor/harness-logo.js';

  import { launchSound } from '$lib/ipc/actions';
  import { motion } from '$lib/stores/motion.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import { intro, INTRO_EXIT_MS, INTRO_LEAD_MS } from './intro.svelte';

  /** Width of the mark on the splash. */
  const MARK_WIDTH = 168;
  /** The finished mark stays a moment before it leaves. */
  const HOLD_MS = 300;

  // Only the full-motion intro shows the logo; reduced motion is a plain crossfade of the shell.
  const visible = $derived(motion.level === 'full' && intro.active);
  const leaving = $derived(intro.phase === 'handoff');

  let logo: HTMLElement | undefined = $state();

  // The launch reports its end through a DOM event (the shared <harness-logo> element), so listen without a Svelte handler.
  $effect(() => {
    const element = logo;
    if (!element) {
      return;
    }

    let hold: ReturnType<typeof setTimeout> | undefined;
    const done = () => (hold = setTimeout(() => intro.launched(), HOLD_MS));
    element.addEventListener('finish', done);

    return () => {
      element.removeEventListener('finish', done);
      clearTimeout(hold);
    };
  });

  // The sound is played natively (the Linux webview blocks audio until a gesture, and a gesture skips the intro);
  // it starts with the animation and stops when the intro is skipped.
  $effect(() => {
    if (visible && (prefs.value?.soundEnabled ?? true)) {
      void launchSound(true);
    }
  });

  function skip(): void {
    if (intro.active) {
      if (intro.phase === 'playing') {
        void launchSound(false);
      }

      intro.skip();
    }
  }
</script>

<!-- Any key or click skips instantly; otherwise the shell takes over as soon as the logo's launch is over. -->
<svelte:window onkeydown={skip} onpointerdown={skip} />

{#if visible}
  <!-- The splash is the page canvas in the current theme, with the short launch of the shared Harness logo. -->
  <div class="pointer-events-none fixed inset-0 z-[100] grid place-items-center" data-leaving={leaving ? '' : undefined} aria-hidden="true">
    <!-- Covers the app while the logo plays, then lifts to reveal the next screen, which enters as the veil goes. -->
    <div class="veil absolute inset-0 bg-canvas" style:--lead="{INTRO_LEAD_MS}ms"></div>

    <div class="lockup relative" style:--exit="{INTRO_EXIT_MS}ms" style:width="{MARK_WIDTH}px">
      <harness-logo bind:this={logo} version="short" autoplay style:width="100%" style:--harness-logo-dust="var(--logo-dust)"></harness-logo>
    </div>
  </div>
{/if}

<style>
  /* The mark leaves first: it fades and grows slightly, so the next screen never overlaps it. */
  .lockup {
    transition:
      opacity var(--exit) var(--ease-out-expo),
      transform var(--exit) var(--ease-out-expo);
  }

  [data-leaving] .lockup {
    opacity: 0;
    transform: scale(1.04);
  }

  [data-leaving] .veil {
    animation: veil-lift 0.42s var(--lead) var(--ease-out-expo) both;
  }

  @keyframes veil-lift {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }
</style>

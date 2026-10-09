<script lang="ts">
  import AuroraCanvas from '$lib/components/AuroraCanvas.svelte';
  import { createSpring, moveSpring } from '$lib/design/motion';
  import { motion } from '$lib/stores/motion.svelte';

  import { intro } from './intro.svelte';

  /** Brightness of the aurora per phase: a barely-there glow during the intro (the logo is the star), a soft glow behind the shell afterwards. */
  const INTENSITY = { idle: 0, playing: 0.35, handoff: 0.65, done: 0.45 } as const;

  const intensity = createSpring(0);
  const phase = $derived(intro.phase);
  const covering = $derived(phase === 'playing');
  // Keeps drifting behind the shell: a frozen frame of the curtains reads as smudges, not light.
  const animated = $derived(motion.level === 'full' && phase !== 'idle');
  // Behind the shell it is ambient: slower and at a capped frame rate, so it costs little GPU time.
  const ambient = $derived(phase === 'done');

  $effect(() => {
    moveSpring(intensity, INTENSITY[phase]);
  });
</script>

<!--
  One aurora for the whole app: it sits above the (hidden) shell while the intro plays,
  then drops behind the shell and dims. In the light theme its opacity token is 0, so it fades away.
-->
<div
  class="pointer-events-none fixed inset-0 transition-opacity duration-[var(--dur-enter)] ease-[var(--ease-out-expo)]"
  class:z-[90]={covering}
  class:z-0={!covering}
  style:opacity={phase === 'idle' ? 0 : covering ? 1 : 'var(--aurora-opacity)'}
>
  <AuroraCanvas class="size-full" intensity={intensity.current} {animated} speed={ambient ? 0.5 : 1} maxFps={ambient ? 30 : Infinity} />

  <!-- During the intro a scrim keeps only a faint hint of the aurora, so nothing competes with the logo. -->
  <div
    class="absolute inset-0 bg-[oklch(0.15_0.02_265)] transition-opacity duration-[var(--dur-enter)] ease-[var(--ease-out-expo)]"
    style:opacity={covering ? 0.86 : 0}
  ></div>
</div>

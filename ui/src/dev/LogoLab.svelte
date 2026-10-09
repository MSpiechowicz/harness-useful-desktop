<script lang="ts">
  import { tick } from 'svelte';

  import Logo from '$lib/components/Logo.svelte';
  import { motion } from '$lib/stores/motion.svelte';

  /** Longest point worth scrubbing to: the launch ends after the second blink (≈3.55 s). */
  const END_MS = 3700;
  const SPEEDS = [0.1, 0.25, 0.5, 1, 2] as const;

  let run = $state(0);
  let loop = $state(true);
  let speed = $state<number>(1);
  let size = $state(96);
  let light = $state(false);
  let scrubbing = $state(false);
  let at = $state(0);
  let stage: HTMLDivElement | undefined = $state();

  /** CSS animations of the playing logo, plus its SVG (the dust filter's noise drifts on the SVG clock). */
  function animations(): Animation[] {
    return stage?.getAnimations({ subtree: true }) ?? [];
  }

  function svgs(): SVGSVGElement[] {
    return [...(stage?.querySelectorAll('svg') ?? [])];
  }

  async function replay(): Promise<void> {
    scrubbing = false;
    run += 1;
    await tick();
    applySpeed();
  }

  function applySpeed(): void {
    for (const animation of animations()) {
      animation.playbackRate = speed;
    }
  }

  // Scrubbing pauses everything at one moment: CSS animations by their clock, the SVG noise by its own.
  function scrubTo(ms: number): void {
    scrubbing = true;
    at = ms;

    for (const animation of animations()) {
      animation.pause();
      animation.currentTime = ms;
    }

    for (const svg of svgs()) {
      svg.pauseAnimations();
      svg.setCurrentTime(ms / 1000);
    }
  }

  function finished(): void {
    if (loop && !scrubbing) {
      setTimeout(() => void replay(), 900 / speed);
    }
  }

  $effect(() => {
    void speed;
    applySpeed();
  });

  // The lab picks its own theme, independent of the app's saved preference.
  $effect(() => {
    document.documentElement.dataset.theme = light ? 'light' : 'dark';
  });
</script>

<main class="lab">
  <header class="bar">
    <strong>Logo lab</strong>
    <button type="button" onclick={() => void replay()}>Replay</button>
    <label><input type="checkbox" bind:checked={loop} /> Loop</label>

    <label>
      Speed
      <select bind:value={speed}>
        {#each SPEEDS as option (option)}
          <option value={option}>{option}×</option>
        {/each}
      </select>
    </label>

    <label>Size <input type="range" min="24" max="200" bind:value={size} /> {size}px</label>
    <label><input type="checkbox" bind:checked={light} /> Light</label>

    <label class="scrub">
      Scrub
      <input type="range" min="0" max={END_MS} step="10" value={at} oninput={(event) => scrubTo(Number(event.currentTarget.value))} />
      <output>{scrubbing ? `${at} ms` : 'playing'}</output>
    </label>
  </header>

  {#if motion.level !== 'full'}
    <p class="note">Motion is “{motion.level}” (app setting or the OS reduced-motion preference), so the logo shows its final state. Set motion to Full to see the launch.</p>
  {/if}

  <div class="stage" bind:this={stage}>
    {#key run}
      <Logo size={size} nameSize={Math.round(size * 0.8)} wordmark animate="launch" onfinish={finished} />
    {/key}
  </div>
</main>

<style>
  .lab {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    color: var(--text);
    font: 13px/1.4 var(--font-ui);
  }

  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 16px;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
  }

  .bar label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }

  .bar button,
  .bar select {
    font: inherit;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    padding: 3px 10px;
  }

  .scrub input {
    width: 260px;
  }

  .scrub output {
    min-width: 64px;
    font-variant-numeric: tabular-nums;
  }

  .note {
    margin: 12px 20px 0;
    color: var(--warning, #e5b44c);
  }

  .stage {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 40px;
  }
</style>

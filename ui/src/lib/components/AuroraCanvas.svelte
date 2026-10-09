<script lang="ts">
  import { createAuroraRenderer, type AuroraRenderer } from './aurora';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** Brightness of the aurora over the base color, 0 to 1. */
    intensity?: number;
    /** Run the animation loop; when false a single static frame is drawn. */
    animated?: boolean;
    class?: string;
  }

  let { intensity = 1, animated = true, class: className }: Props = $props();

  /** Time offset of the still frame, chosen to show an attractive composition. */
  const STATIC_SECONDS = 7;

  let canvas: HTMLCanvasElement | undefined = $state();
  let renderer: AuroraRenderer | null = $state.raw(null);
  let contextLost = $state(false);
  let ready = $state(false);

  // Create the GL renderer once the canvas exists; no WebGL2 leaves `renderer` null (CSS fallback shows).
  $effect(() => {
    if (!canvas) {
      return;
    }

    const created = createAuroraRenderer(canvas, (lost) => (contextLost = lost));
    const observer = new ResizeObserver(([entry]) => {
      created?.resize(entry.contentRect.width, entry.contentRect.height);
      created?.render(STATIC_SECONDS, intensity);
    });
    observer.observe(canvas);
    renderer = created;

    return () => {
      observer.disconnect();
      created?.dispose();
      renderer = null;
    };
  });

  // Animated: rAF loop reading the latest intensity. Still: one frame per intensity change.
  $effect(() => {
    if (!renderer) {
      return;
    }

    if (!animated) {
      renderer.render(STATIC_SECONDS, intensity);
      ready = true;

      return;
    }

    const active = renderer;
    const started = performance.now();
    let frame = 0;

    const tick = (now: number): void => {
      if (!document.hidden) {
        active.render(STATIC_SECONDS + (now - started) / 1000, intensity);
        ready = true;
      }

      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(frame);
  });
</script>

<div class={cx('relative overflow-hidden bg-[oklch(0.15_0.02_265)]', className)} aria-hidden="true">
  <!-- Always-present fallback; also what shows before the first GL frame and after context loss. -->
  <div
    class="absolute inset-0"
    style:background="radial-gradient(60% 50% at 25% 30%, oklch(0.5 0.2 295 / 0.55), transparent 70%), radial-gradient(55% 45% at 75% 65%, oklch(0.55 0.15 220 / 0.45), transparent 70%)"
  ></div>

  <canvas
    bind:this={canvas}
    class={cx('absolute inset-0 size-full transition-opacity duration-[var(--dur-slow)]', ready && !contextLost ? 'opacity-100' : 'opacity-0')}
  ></canvas>
</div>

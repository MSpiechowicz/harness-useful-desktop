<svelte:options namespace="svg" />

<script lang="ts">
  import { CHIPS, CRACK, GRIT, PUFFS } from './launch-parts';
  import { CURSOR, H_BROKEN, H_INTACT, LEG_PIVOT } from './geometry';

  interface Props {
    /** Gradient painting the whole mark; defined inside the same `<svg>` so theme variables resolve. */
    gradient: string;
    /** The gradient as the leg sees it before landing (turned with the leg, so no seam shows at the snap). */
    legGradient: string;
    /** Blur filter that softens the dust. */
    blur: string;
    /** Called once the cursor has blinked twice, the last thing the launch does. */
    onfinish: () => void;
  }

  let { gradient, legGradient, blur, onfinish }: Props = $props();

  // The cursor's own blink is the last animation; its children's animations bubble up and are ignored.
  function onLegEnd(event: AnimationEvent): void {
    if (event.target === event.currentTarget) {
      onfinish();
    }
  }
</script>

<!-- Intact H until the snap; then the broken H and the separate leg take over in the same frame. -->
<g class="body" fill="url(#{gradient})">
  <path class="intact" d={H_INTACT} />
  <path class="broken" d={H_BROKEN} />
</g>

<g filter="url(#{blur})" style:fill="var(--logo-dust)">
  {#each PUFFS as puff, index (index)}
    <circle
      class={['puff', puff.slow && 'slow']}
      cx={puff.cx}
      cy={puff.cy}
      r={puff.r}
      style:--dx="{puff.dx}px"
      style:--dy="{puff.dy}px"
      style:--s={puff.scale}
      style:--d="{puff.delay}s"
    />
  {/each}
</g>

<g style:fill="var(--text)">
  {#each CHIPS as chip, index (index)}
    <rect
      class="fleck"
      x={chip.x}
      y={chip.y}
      width={chip.size}
      height={chip.size}
      style:--dx="{chip.dx}px"
      style:--h="{chip.rise}px"
      style:--d="{chip.delay}s"
    />
  {/each}
</g>

<g class="leg-visible">
  <g class="cursor" data-part="cursor" onanimationend={onLegEnd}>
    <g class="leg" style:transform-origin="{LEG_PIVOT.x}px {LEG_PIVOT.y}px">
      <rect class="leg-base" {...CURSOR} fill="url(#{legGradient})" />
      <rect class="leg-color" {...CURSOR} fill="url(#{gradient})" />
    </g>
  </g>
</g>

<g fill="none" stroke-linecap="round" stroke-linejoin="round" style:stroke="var(--canvas)">
  <path class="crack" pathLength="1" stroke-width={CRACK.main.width} d={CRACK.main.d} />
  <path class="crack branch" pathLength="1" stroke-width={CRACK.branch.width} d={CRACK.branch.d} />
</g>

<g style:fill="var(--text)">
  {#each GRIT as grit, index (index)}
    <rect class="grit" x={grit.x} y={grit.y} width={grit.size} height={grit.size} style:--dx="{grit.dx}px" style:--d="{grit.delay}s" />
  {/each}
</g>

<style>
  /* Timeline in seconds from mount (approved launch, slower crack): crack .3, shiver .95, snap 1.25, impact 1.85, blinks 2.45. */

  /* Crack draws slowly across the joint, grit trickles out, the whole glyph shivers just before the snap. */
  .crack {
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    animation:
      crack-draw 0.55s 0.3s cubic-bezier(0.6, 0, 0.4, 1) forwards,
      crack-gone 0.01s 1.33s forwards;
  }

  .crack.branch {
    animation:
      crack-draw 0.32s 0.68s cubic-bezier(0.6, 0, 0.4, 1) forwards,
      crack-gone 0.01s 1.33s forwards;
  }

  .grit,
  .puff,
  .fleck {
    transform-box: fill-box;
    transform-origin: center;
    opacity: 0;
  }

  .grit {
    animation: grit 0.8s calc(0.48s + var(--d, 0s)) cubic-bezier(0.5, 0, 1, 1) both;
  }

  .body {
    transform-box: view-box;
    animation:
      shiver 0.3s 0.95s linear both,
      shudder 0.22s 1.85s ease-out both;
  }

  /* The snap: the intact H is replaced by the broken H and the separate leg in the same frame. */
  .intact {
    animation: swap-out 0.01s 1.25s forwards;
  }

  .broken,
  .leg-visible {
    opacity: 0;
    animation: swap-in 0.01s 1.25s forwards;
  }

  /* The leg topples 90 degrees about its bottom-right corner, lands with a small bounce, and gains its final colour. */
  .leg {
    transform-box: view-box;
    animation: topple 0.78s 1.25s cubic-bezier(0.55, 0, 1, 0.5) both;
  }

  .leg-color {
    opacity: 0;
    animation: fade-in 0.45s 1.9s ease-out forwards;
  }

  .leg-base {
    animation: fade-out 0.45s 1.9s ease-out forwards;
  }

  .cursor {
    animation: blink-twice 1.1s 2.45s steps(1, end) both;
  }

  /* Impact: dust puffs, pixel chips, a shudder. */
  .puff {
    animation: puff 0.95s calc(1.85s + var(--d, 0s)) cubic-bezier(0.12, 0.7, 0.25, 1) both;
  }

  .puff.slow {
    animation-duration: 1.3s;
  }

  .fleck {
    animation: fleck 0.7s calc(1.85s + var(--d, 0s)) linear both;
  }

  @keyframes crack-draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  @keyframes crack-gone {
    to {
      opacity: 0;
    }
  }

  @keyframes grit {
    0% {
      opacity: 0;
      transform: translate(0, 0);
    }
    1% {
      opacity: 1;
      transform: translate(0, 0);
    }
    100% {
      opacity: 0;
      transform: translate(var(--dx), 26px) rotate(140deg);
    }
  }

  @keyframes shiver {
    0%,
    100% {
      transform: translate(0, 0);
    }
    20% {
      transform: translate(0.6px, 0);
    }
    40% {
      transform: translate(-0.6px, 0.2px);
    }
    60% {
      transform: translate(0.5px, -0.2px);
    }
    80% {
      transform: translate(-0.3px, 0);
    }
  }

  @keyframes shudder {
    0% {
      transform: translateY(0);
    }
    30% {
      transform: translateY(1.6px);
    }
    65% {
      transform: translateY(-0.6px);
    }
    100% {
      transform: translateY(0);
    }
  }

  @keyframes swap-out {
    to {
      opacity: 0;
    }
  }

  @keyframes swap-in {
    to {
      opacity: 1;
    }
  }

  @keyframes topple {
    0% {
      transform: rotate(-90deg);
    }
    78% {
      transform: rotate(0deg);
      animation-timing-function: cubic-bezier(0.2, 0.8, 0.3, 1);
    }
    88% {
      transform: rotate(-7deg);
      animation-timing-function: cubic-bezier(0.5, 0, 0.8, 0.4);
    }
    100% {
      transform: rotate(0deg);
    }
  }

  @keyframes fade-in {
    to {
      opacity: 1;
    }
  }

  @keyframes fade-out {
    to {
      opacity: 0;
    }
  }

  @keyframes blink-twice {
    0%,
    24% {
      opacity: 1;
    }
    25%,
    49% {
      opacity: 0;
    }
    50%,
    74% {
      opacity: 1;
    }
    75%,
    99% {
      opacity: 0;
    }
    100% {
      opacity: 1;
    }
  }

  @keyframes puff {
    0% {
      opacity: 0;
      transform: translate(0, 0) scale(0.25);
    }
    12% {
      opacity: 0.8;
    }
    50% {
      opacity: 0.4;
    }
    100% {
      opacity: 0;
      transform: translate(var(--dx), var(--dy)) scale(var(--s));
    }
  }

  @keyframes fleck {
    0% {
      opacity: 0;
      transform: translate(0, 0) rotate(0deg);
    }
    1% {
      opacity: 1;
      transform: translate(0, 0) rotate(0deg);
    }
    45% {
      opacity: 1;
      transform: translate(calc(var(--dx) * 0.55), var(--h)) rotate(90deg);
      animation-timing-function: cubic-bezier(0.5, 0, 1, 1);
    }
    100% {
      opacity: 0;
      transform: translate(var(--dx), 5px) rotate(200deg);
    }
  }
</style>

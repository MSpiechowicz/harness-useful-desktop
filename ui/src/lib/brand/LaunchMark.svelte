<svelte:options namespace="svg" />

<script lang="ts">
  import { CHIPS, CRACK, GRIT, PUFFS, SHARDS, SNAP_PUFFS } from './launch-parts';
  import { CURSOR, H_BROKEN, H_INTACT, LEG_PIVOT, STEM_PIECE } from './geometry';

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

<!--
  Intact H until the crack has run across; then the H with its splintered edge and the stem piece below it,
  which gives a little; at the snap the piece is replaced by the leg, which carries the matching edge.
-->
<g class="body" fill="url(#{gradient})">
  <path class="broken" d={H_BROKEN} />
  <path class="piece" d={STEM_PIECE} />
  <path class="intact" d={H_INTACT} />
</g>

<g filter="url(#{blur})" style:fill="var(--logo-dust)">
  {#each SNAP_PUFFS as puff, index (index)}
    <circle class="puff snap" cx={puff.cx} cy={puff.cy} r={puff.r} style:--dx="{puff.dx}px" style:--dy="{puff.dy}px" style:--s={puff.scale} style:--d="{puff.delay}s" />
  {/each}

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

<g fill="url(#{gradient})">
  {#each CHIPS as chip, index (index)}
    <g transform="translate({chip.x} {chip.y}) scale({chip.scale})">
      <path class="fleck" d={SHARDS[chip.shard]} style:--dx="{chip.dx / chip.scale}px" style:--h="{chip.rise / chip.scale}px" style:--d="{chip.delay}s" />
    </g>
  {/each}
</g>

<g class="leg-visible">
  <g class="cursor" data-part="cursor" onanimationend={onLegEnd}>
    <g class="leg" style:transform-origin="{LEG_PIVOT.x}px {LEG_PIVOT.y}px">
      <path class="leg-base" d={CURSOR} fill="url(#{legGradient})" />
      <path class="leg-color" d={CURSOR} fill="url(#{gradient})" />
    </g>
  </g>
</g>

<path class="crack" pathLength="1" fill="none" stroke-linecap="butt" stroke-linejoin="miter" style:stroke="var(--canvas)" stroke-width={CRACK.width} d={CRACK.d} />

<g fill="url(#{gradient})">
  {#each GRIT as grit, index (index)}
    <g transform="translate({grit.x} {grit.y}) scale({grit.scale})">
      <path class="grit" d={SHARDS[grit.shard]} style:--dx="{grit.dx / grit.scale}px" style:--d="{grit.delay}s" />
    </g>
  {/each}
</g>

<style>
  /*
   * Timeline in seconds from mount: the crack runs .3-1.0, the stem piece gives at 1.02, shiver .98, snap 1.25
   * (with a puff of dust from the crack), impact 1.85, blinks 2.45.
   */

  /* A thin, sharp crack runs fast, then slows as it reaches the inner edge; grit trickles out of it. */
  .crack {
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    animation:
      crack-draw 0.7s 0.3s cubic-bezier(0.2, 0.8, 0.3, 1) forwards,
      crack-gone 0.01s 1.02s forwards;
  }

  .grit,
  .puff,
  .fleck {
    transform-box: fill-box;
    transform-origin: center;
    opacity: 0;
  }

  .grit {
    animation: grit 0.8s calc(0.55s + var(--d, 0s)) cubic-bezier(0.5, 0, 1, 1) both;
  }

  .body {
    transform-box: view-box;
    animation:
      shiver 0.26s 0.98s linear both,
      shudder 0.22s 1.85s ease-out both;
  }

  /* The crack opens: the intact H gives way to the splintered H and the stem piece, which sags a hair. */
  .intact {
    animation: swap-out 0.01s 1.02s forwards;
  }

  .piece {
    transform-box: view-box;
    opacity: 0;
    animation:
      swap-in 0.01s 1.02s forwards,
      give 0.25s 1.02s cubic-bezier(0.5, 0, 0.8, 0.4) forwards,
      swap-out 0.01s 1.25s forwards;
  }

  /* The snap: the piece is replaced by the leg, which carries the same splintered edge, in the same frame. */
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

  /* Impact: dust puffs (rollers keep drifting along the ground), splinters, a shudder. */
  .puff {
    animation: puff 0.95s calc(1.85s + var(--d, 0s)) cubic-bezier(0.12, 0.7, 0.25, 1) both;
  }

  .puff.slow {
    animation-duration: 1.3s;
  }

  .puff.snap {
    animation: puff 0.8s calc(1.25s + var(--d, 0s)) cubic-bezier(0.12, 0.7, 0.25, 1) both;
  }

  .fleck {
    animation: fleck 0.7s calc(1.85s + var(--d, 0s)) linear both;
  }

  @keyframes give {
    to {
      transform: translate(0.35px, 0.9px);
    }
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
    /* Starts where the stem piece sagged to, so nothing jumps at the snap. */
    0% {
      transform: rotate(-90deg) translate(-0.9px, 0.35px);
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

<script lang="ts">
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  interface Props {
    /** Mark size in px. */
    size?: number;
    wordmark?: boolean;
    layout?: 'inline' | 'stacked';
    /** Play the reveal: stroke draw, fill fade, glow pulse, staggered wordmark. */
    animate?: boolean;
    class?: string;
  }

  let { size = 28, wordmark = false, layout = 'inline', animate = false, class: className }: Props = $props();

  const gradientId = uid('logo-gradient');
  const NAME = 'Harness Desktop';
  const letters = Array.from(NAME);
</script>

<span
  class={cx('inline-flex items-center', layout === 'stacked' ? 'flex-col gap-4' : 'gap-2.5', className)}
  class:animate
  role="img"
  aria-label={NAME}
>
  <!-- Mark: two harness rails joined through an orbiting node. -->
  <svg class="mark" width={size} height={size} viewBox="0 0 64 64" fill="none" aria-hidden="true">
    <defs>
      <linearGradient id={gradientId} x1="8" y1="6" x2="56" y2="58" gradientUnits="userSpaceOnUse">
        <stop offset="0" stop-color="var(--accent-1)" />
        <stop offset="0.55" stop-color="var(--accent-2)" />
        <stop offset="1" stop-color="var(--accent-3)" />
      </linearGradient>
    </defs>

    <g stroke="url(#{gradientId})" stroke-width="4.5" stroke-linecap="round">
      <path class="draw" style:--i="0" pathLength="1" d="M19 11V53" />
      <path class="draw" style:--i="1" pathLength="1" d="M45 11V53" />
      <path class="draw" style:--i="2" pathLength="1" d="M19 32H24.5M39.5 32H45" />
      <circle class="draw" style:--i="3" pathLength="1" cx="32" cy="32" r="7.5" />
    </g>
    <ellipse
      class="draw orbit"
      style:--i="4"
      pathLength="1"
      cx="32"
      cy="32"
      rx="17"
      ry="6.5"
      transform="rotate(-28 32 32)"
      stroke="url(#{gradientId})"
      stroke-width="1.6"
      stroke-linecap="round"
    />
    <circle class="node" cx="32" cy="32" r="3.2" fill="url(#{gradientId})" />
  </svg>

  {#if wordmark}
    <span
      class="wordmark font-semibold tracking-tight text-fg"
      style:font-size="{layout === 'stacked' ? size * 0.34 : size * 0.62}px"
      aria-hidden="true"
    >
      {#each letters as letter, index (index)}<span class="letter" style:--i={index}>{letter === ' ' ? ' ' : letter}</span>{/each}
    </span>
  {/if}
</span>

<style>
  /* Reveal timeline in ms from when `.animate` is applied (total under 1.2 s). */
  .animate .draw {
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    animation: draw 520ms var(--ease-out-expo) forwards;
    animation-delay: calc(80ms + var(--i) * 90ms);
  }

  .animate .node {
    transform-origin: 32px 32px;
    animation: node-in 380ms var(--ease-spring) 640ms both;
  }

  .animate .mark {
    animation: glow 520ms ease-out 760ms both;
  }

  .animate .letter {
    display: inline-block;
    animation: letter-in 340ms var(--ease-out-expo) both;
    animation-delay: calc(560ms + var(--i) * 22ms);
  }

  .orbit {
    opacity: 0.8;
  }

  @keyframes draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  @keyframes node-in {
    from {
      opacity: 0;
      transform: scale(0);
    }
  }

  @keyframes glow {
    50% {
      filter: drop-shadow(0 0 14px var(--accent-1)) drop-shadow(0 0 28px var(--accent-2));
    }
  }

  @keyframes letter-in {
    from {
      opacity: 0;
      transform: translateY(0.4em);
      filter: blur(4px);
    }
  }
</style>

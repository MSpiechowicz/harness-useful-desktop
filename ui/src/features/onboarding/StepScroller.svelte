<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { motion } from '$lib/stores/motion.svelte';

  interface Props {
    /** Changes when another step takes over, so the new one starts at its top. */
    resetKey: unknown;
    children: Snippet;
  }

  let { resetKey, children }: Props = $props();

  /** How close to the top counts as being at it, in px. */
  const EDGE = 4;

  /** Content hidden below the fold has to exceed the area's own bottom padding by this much to count: padding alone is not "more". */
  const SLIVER = 2;

  let area = $state<HTMLElement>();
  let below = $state(false);
  let above = $state(false);

  function measure(): void {
    if (!area) {
      return;
    }

    const hidden = area.scrollHeight - area.scrollTop - area.clientHeight - parseFloat(getComputedStyle(area).paddingBottom);

    below = hidden > SLIVER;
    above = area.scrollTop > EDGE;
  }

  /** One page down, minus a little so the line that was last stays in view. */
  function page(): void {
    area?.scrollBy({ top: Math.max(area.clientHeight - 48, 48), behavior: motion.level === 'full' ? 'smooth' : 'auto' });
  }

  // The content grows and shrinks by itself (data arriving, a disclosure opening, the window resizing, a step leaving), so the
  // edges are measured again whenever the area or anything directly in it changes size or is added or removed.
  $effect(() => {
    if (!area) {
      return;
    }

    const node = area;
    const sizes = new ResizeObserver(measure);

    function watch(): void {
      sizes.disconnect();
      sizes.observe(node);

      for (const child of node.children) {
        sizes.observe(child);
      }

      measure();
    }

    const members = new MutationObserver(watch);
    members.observe(node, { childList: true });
    watch();

    return () => {
      sizes.disconnect();
      members.disconnect();
    };
  });

  $effect(() => {
    void resetKey;

    if (area) {
      area.scrollTop = 0;
    }

    measure();
  });
</script>

<!-- Stacked grid cell: the outgoing and incoming steps overlap while they slide. -->
<div class="relative -m-6 flex min-h-0 flex-1 flex-col">
  <div
    bind:this={area}
    data-above={above}
    data-below={below}
    onscroll={measure}
    class="step-area grid min-h-0 flex-1 overflow-x-hidden overflow-y-auto p-6 *:col-start-1 *:row-start-1"
  >
    {@render children()}
  </div>

  <!-- Over the faded bottom edge while there is more to see, gone at the end. Pointer only: the keys that scroll the area are the way for the keyboard. The same button as the chat's "Jump to latest". -->
  <div class="more-cue pointer-events-none absolute inset-x-0 bottom-4 flex justify-center" data-visible={below}>
    <Button class="pointer-events-auto shadow-float" tabindex={-1} aria-label={t('onboarding.scroll.more')} onclick={page}>
      {#snippet leading()}<ArrowDown class="more-arrow" size={16} aria-hidden="true" />{/snippet}
      {t('onboarding.scroll.more')}
    </Button>
  </div>
</div>

<style>
  /*
   * No scrollbar line beside the step: an edge that has more beyond it fades instead, and the fade eases in and out
   * with the scroll position (the two lengths are registered properties, so the mask can transition).
   */
  .step-area {
    --fade-top: 0px;
    --fade-bottom: 0px;
    scrollbar-width: none;
    -webkit-mask-image: linear-gradient(to bottom, transparent, black var(--fade-top), black calc(100% - var(--fade-bottom)), transparent);
    mask-image: linear-gradient(to bottom, transparent, black var(--fade-top), black calc(100% - var(--fade-bottom)), transparent);
    transition:
      --fade-top var(--dur-base) var(--ease-out-expo),
      --fade-bottom var(--dur-base) var(--ease-out-expo);
  }

  .step-area[data-above='true'] {
    --fade-top: 28px;
  }

  .step-area[data-below='true'] {
    --fade-bottom: 44px;
  }

  .step-area::-webkit-scrollbar {
    display: none;
  }

  .more-cue {
    transition:
      opacity var(--dur-base) var(--ease-out-expo),
      transform var(--dur-base) var(--ease-out-expo),
      background-color var(--dur-fast),
      color var(--dur-fast),
      visibility 0s linear var(--dur-base);
    visibility: hidden;
    opacity: 0;
    transform: translateY(calc(var(--shift) / 2));
  }

  .more-cue[data-visible='true'] {
    visibility: visible;
    opacity: 1;
    transform: none;
    transition-delay: 0s;
  }

  .more-cue :global(.more-arrow) {
    animation: more-bob 1.8s ease-in-out infinite;
  }

  @keyframes more-bob {
    0%,
    100% {
      transform: translateY(-1px);
    }

    50% {
      transform: translateY(1.5px);
    }
  }

  /* Still for whoever asked for less motion. */
  @media (prefers-reduced-motion: reduce) {
    .more-cue :global(.more-arrow) {
      animation: none;
    }
  }

  :global(:root[data-motion='reduced']) .more-cue :global(.more-arrow),
  :global(:root[data-motion='off']) .more-cue :global(.more-arrow) {
    animation: none;
  }
</style>

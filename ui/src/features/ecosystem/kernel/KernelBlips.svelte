<script lang="ts">
  import { cx } from '$lib/utils/cx';

  import { blipAt } from './kernel-blips';
  import { pointAlong } from './kernel-layout';
  import type { WiredProps } from './kernel-wires';

  let { layout, members, hot }: WiredProps = $props();

  let elements = $state<(HTMLElement | undefined)[]>([]);

  /** Data now and then crossing a loaded module's trace, moved with a transform so the page never lays out again. */
  $effect(() => {
    const start = performance.now();
    let frame = 0;

    const tick = () => {
      const seconds = (performance.now() - start) / 1000;

      for (const slot of layout.modules) {
        const element = elements[slot.index];
        const member = members[slot.index];

        if (!element || !member) {
          continue;
        }

        const blip = blipAt(slot.index, member.state, seconds);

        if (blip === null) {
          element.style.setProperty('--opacity', '0');
          continue;
        }

        const [x, y] = pointAlong(slot.route, blip.u);

        element.style.transform = `translate(${x}px, ${y}px)`;
        element.style.setProperty('--opacity', blip.opacity.toFixed(2));
      }

      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(frame);
  });
</script>

{#each layout.modules as slot (slot.index)}
  {@const member = members[slot.index]}

  {#if member.state !== 'missing'}
    <span bind:this={elements[slot.index]} class={cx('blip', member.state, hot !== null && hot !== member.id && 'dim')} data-blip={member.id} style:--mc={member.colour}></span>
  {/if}
{/each}

<style>
  /* Anchored on the stage's origin and moved from there. */
  .blip {
    position: absolute;
    top: calc(var(--kernel-blip) / -2);
    left: calc(var(--kernel-blip) / -2);
    z-index: 2;
    width: var(--kernel-blip);
    height: var(--kernel-blip);
    border-radius: calc(var(--kernel-blip) / 3);
    background: var(--mc);
    box-shadow: 0 0 var(--kernel-blip) color-mix(in srgb, var(--mc) 60%, transparent);
    opacity: var(--opacity, 0);
    will-change: transform;
    pointer-events: none;
  }

  .blip.update {
    background: var(--accent-2);
    box-shadow: 0 0 var(--kernel-blip) color-mix(in srgb, var(--accent-2) 60%, transparent);
  }

  .blip.dim {
    opacity: 0;
  }
</style>

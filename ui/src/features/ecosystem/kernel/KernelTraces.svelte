<script lang="ts">
  import { cx } from '$lib/utils/cx';

  import { pathData } from './kernel-layout';
  import type { WiredProps } from './kernel-wires';

  let { layout, members, hot }: WiredProps = $props();

  const VIA_RADIUS = 2.5;
  const anyLoaded = $derived(members.some((member) => member.state !== 'missing'));
</script>

<svg class="traces" width={layout.width} height={layout.height} aria-hidden="true">
  {#if layout.trunk}
    <path class={cx('trace trunk', !anyLoaded && 'missing')} d={pathData(layout.trunk)} />
  {/if}

  {#each layout.modules as slot (slot.index)}
    {@const member = members[slot.index]}
    {@const end = slot.path[slot.path.length - 1]}

    <g class={cx('module', hot !== null && hot !== member.id && 'dim')} data-trace={member.id} style:--mc={member.colour}>
      <path class={cx('trace', member.state, hot === member.id && 'hot')} d={pathData(slot.path)} />
      <!-- The wide layout ends every trace in a via on the chip's pin. -->
      {#if !layout.narrow}
        <circle class={cx('via', member.state)} cx={end[0]} cy={end[1]} r={VIA_RADIUS} />
      {/if}
    </g>
  {/each}
</svg>

<style>
  .traces {
    position: absolute;
    inset: 0;
    overflow: visible;
    pointer-events: none;
  }

  .module {
    transition: opacity var(--dur-base);
  }

  .module.dim {
    opacity: 0.38;
  }

  .trace {
    fill: none;
    stroke: var(--mc);
    stroke-opacity: 0.7;
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
    transition:
      stroke-opacity var(--dur-base),
      stroke-width var(--dur-base);
  }

  .trace.hot {
    stroke-opacity: 1;
    stroke-width: 3;
  }

  .trace.update {
    stroke: var(--accent-2);
  }

  .trace.missing,
  .trace.trunk.missing {
    stroke: var(--text-3);
    stroke-dasharray: 3 4;
    stroke-width: 1.5;
    stroke-opacity: 0.55;
  }

  .trace.trunk {
    stroke: var(--accent-2);
    stroke-opacity: 0.55;
  }

  .via {
    fill: var(--mc);
    fill-opacity: 0.8;
  }

  .via.update {
    fill: var(--accent-2);
  }

  .via.missing {
    fill: var(--text-3);
    fill-opacity: 0.5;
  }
</style>

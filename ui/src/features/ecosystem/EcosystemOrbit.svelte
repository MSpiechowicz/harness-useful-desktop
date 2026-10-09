<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Plus from '@lucide/svelte/icons/plus';

  import { ICON } from '$lib/components/icon';
  import Logo from '$lib/components/Logo.svelte';
  import { rise } from '$lib/design/motion';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';

  import { ECOSYSTEM_ICONS, installedSummary, itemState, SHORT_NAMES } from './ecosystem-items';

  interface Props {
    items: EcosystemItem[];
  }

  let { items }: Props = $props();

  // The three companions sit on a triangle around the app, in the 320 x 200 box of the lines.
  const SPOTS = [
    { x: 56, y: 56 },
    { x: 264, y: 56 },
    { x: 160, y: 168 },
  ] as const;
  const CENTER = { x: 160, y: 92 } as const;

  const description = $derived(
    `${installedSummary(items)}: ${items.map((item) => `${item.name} ${itemState(item) === 'installed' ? 'installed' : 'not installed'}`).join(', ')}`,
  );
</script>

<div role="img" aria-label={description} class="relative mx-auto aspect-[8/5] w-full max-w-[22rem]">
  <svg viewBox="0 0 320 200" class="absolute inset-0 size-full overflow-visible" aria-hidden="true">
    {#each items.slice(0, SPOTS.length) as item, index (item.id)}
      {@const spot = SPOTS[index]}
      {@const installed = itemState(item) === 'installed'}

      <line
        x1={CENTER.x}
        y1={CENTER.y}
        x2={spot.x}
        y2={spot.y}
        stroke={installed ? 'var(--accent-2)' : 'var(--text-3)'}
        stroke-width={installed ? 2 : 1.5}
        stroke-dasharray={installed ? undefined : '3 6'}
        stroke-linecap="round"
        opacity={installed ? 0.8 : 0.6}
      />
    {/each}
  </svg>

  <!-- Pings behind the mark only while motion is full; the state itself never depends on it. -->
  <span
    class="absolute grid size-14 -translate-x-1/2 -translate-y-1/2 place-items-center"
    style:left="{(CENTER.x / 320) * 100}%"
    style:top="{(CENTER.y / 200) * 100}%"
  >
    {#if motion.level === 'full'}
      <span class="absolute inset-0 animate-ping rounded-full bg-accent-wash [animation-duration:3.2s]"></span>
    {/if}
    <span class="relative grid size-14 place-items-center rounded-full border border-line-strong bg-surface-2 shadow-float"><Logo variant="mark" size={26} /></span>
  </span>

  {#each items.slice(0, SPOTS.length) as item, index (item.id)}
    {@const spot = SPOTS[index]}
    {@const status = itemState(item)}
    {@const Icon = ECOSYSTEM_ICONS[item.id]}

    <div
      in:rise={{ y: 10, delay: 120 + index * 90, duration: 480 }}
      class="absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center gap-1"
      style:left="{(spot.x / 320) * 100}%"
      style:top="{(spot.y / 200) * 100}%"
    >
      <span
        class={cx(
          'relative grid size-11 place-items-center rounded-full border bg-surface-1 shadow-raised',
          status === 'installed' ? 'border-accent/50 text-accent-text' : 'border-dashed border-line-strong text-fg-3',
        )}
      >
        <Icon size={ICON.lg} aria-hidden="true" />
        <span
          class={cx(
            'absolute -right-1 -bottom-1 grid size-4 place-items-center rounded-full border border-surface-1',
            status === 'installed' ? 'bg-success text-surface-1' : 'bg-surface-4 text-fg-2',
          )}
        >
          {#if status === 'installed'}<Check size={12} strokeWidth={3} aria-hidden="true" />{:else}<Plus size={12} strokeWidth={3} aria-hidden="true" />{/if}
        </span>
      </span>
      <span class="text-xs font-medium whitespace-nowrap text-fg-2">{SHORT_NAMES[item.id]}</span>
    </div>
  {/each}
</div>

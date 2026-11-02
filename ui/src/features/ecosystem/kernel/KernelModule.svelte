<script lang="ts">
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import Plus from '@lucide/svelte/icons/plus';

  import Logo from '$lib/components/Logo.svelte';
  import { cx } from '$lib/utils/cx';

  import KernelLed from './KernelLed.svelte';
  import type { ModuleSlot } from './kernel-layout';
  import { moduleLabel, type KernelMember } from './kernel-members';

  interface Props {
    member: KernelMember;
    slot: ModuleSlot;
    animated: boolean;
    /** The stacked layout draws its marks a little smaller. */
    compact: boolean;
    /** Another module is hovered or focused: this one steps back. */
    dimmed: boolean;
    hot: boolean;
    onhot: (hot: boolean) => void;
    onselect: () => void;
  }

  let { member, slot, animated, compact, dimmed, hot, onhot, onselect }: Props = $props();

  const MARK = 18;
  const COMPACT_MARK = 16;
  const TICKS = [0, 1, 2] as const;

  const missing = $derived(member.state === 'missing');
  const markSize = $derived(compact ? COMPACT_MARK : MARK);
</script>

<button
  type="button"
  class={cx('socket', slot.side, member.state, dimmed && 'dim', hot && 'hot')}
  data-module={member.id}
  data-state={member.state}
  aria-label={moduleLabel(member)}
  style:--mc={member.colour}
  style:left="{slot.x}px"
  style:top="{slot.y}px"
  style:width="{slot.w}px"
  style:height="{slot.h}px"
  onpointerenter={() => onhot(true)}
  onpointerleave={() => onhot(false)}
  onfocus={() => onhot(true)}
  onblur={() => onhot(false)}
  onclick={onselect}
>
  <span class="card">
    {#if missing}
      <Plus size={markSize - 2} strokeWidth={2.6} aria-hidden="true" />
    {:else}
      <span class="mark" aria-hidden="true"><Logo variant="mark" product={member.id} size={markSize} /></span>
    {/if}
    <span class="name">{member.short}</span>
  </span>

  <span class="strip" aria-hidden="true">
    <KernelLed tone={missing ? 'empty' : member.state === 'update' ? 'update' : 'member'} pulse={animated && member.state === 'update'} />
    {#each TICKS as tick (tick)}<i class="tick"></i>{/each}
  </span>

  {#if member.state === 'update'}
    <span class="badge" aria-hidden="true"><ArrowUp size={10} strokeWidth={3.4} /></span>
  {/if}
</button>

<style>
  .socket {
    position: absolute;
    z-index: 3;
    display: flex;
    gap: var(--kernel-socket-gap);
    padding: var(--kernel-socket-pad);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    text-align: start;
    cursor: pointer;
    transition:
      border-color var(--dur-base),
      opacity var(--dur-base);
  }

  .socket.right {
    flex-direction: row-reverse;
  }

  .socket.dim {
    opacity: 0.38;
  }

  .socket.hot,
  .socket:focus-visible {
    border-color: var(--mc);
  }

  .card {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: center;
    gap: var(--kernel-card-gap);
    padding-inline: var(--kernel-card-pad);
    border-radius: var(--kernel-card-radius);
    border: 1px solid color-mix(in srgb, var(--mc) 50%, var(--border-strong));
    background: color-mix(in srgb, var(--mc) 10%, var(--surface-2));
    font-size: var(--text-xs);
    font-weight: 600;
    line-height: 1.35;
    white-space: nowrap;
  }

  .mark {
    display: flex;
    flex: none;
  }

  .strip {
    display: flex;
    width: var(--kernel-strip-w);
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    padding-block: var(--kernel-socket-pad);
  }

  .tick {
    display: block;
    width: var(--kernel-tick-w);
    height: var(--kernel-tick-h);
    border-radius: 1px;
    background: var(--mc);
  }

  /* An empty socket: a dashed outline, a plus and the name, nothing lit. */
  .missing {
    border-style: dashed;
    background: transparent;
  }

  .missing .card {
    border: 0;
    background: transparent;
    color: var(--text-3);
  }

  .missing .tick {
    background: var(--text-3);
    opacity: 0.5;
  }

  .missing.hot,
  .missing:focus-visible {
    border-color: var(--text-3);
  }

  .badge {
    position: absolute;
    top: var(--kernel-badge-out);
    inset-inline-start: var(--kernel-badge-out);
    display: grid;
    width: var(--kernel-badge);
    height: var(--kernel-badge);
    place-items: center;
    border-radius: 50%;
    background: var(--accent-fill);
    color: var(--on-accent);
    box-shadow: 0 0 0 2px var(--surface-1);
  }

  .right .badge {
    inset-inline-start: auto;
    inset-inline-end: var(--kernel-badge-out);
  }
</style>

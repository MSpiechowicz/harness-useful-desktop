<script lang="ts">
  import { FAMILY_NAMES } from '$lib/brand/family';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
  import { motion } from '$lib/stores/motion.svelte';
  import { updates } from '$lib/stores/updates.svelte';

  import { cardTitleId } from './ecosystem-items';
  import KernelBlips from './kernel/KernelBlips.svelte';
  import KernelChip from './kernel/KernelChip.svelte';
  import { layoutKernel } from './kernel/kernel-layout';
  import { focusLine, kernelMembers, kernelSummary, type KernelMember } from './kernel/kernel-members';
  import KernelModule from './kernel/KernelModule.svelte';
  import KernelStatus from './kernel/KernelStatus.svelte';
  import KernelTraces from './kernel/KernelTraces.svelte';

  interface Props {
    items: EcosystemItem[];
    /** The room is short: the modules stack in two columns under the chip instead of flanking it. */
    narrow?: boolean;
  }

  let { items, narrow = false }: Props = $props();

  // The diagram of the Harness family. Harness Desktop is the kernel chip in the middle and every member a module seated in
  // a socket beside it: lit in its own colour when loaded, a blue badge when an update waits, an empty dashed socket when
  // it is not installed. Hovering or focusing one traces its wire and reads its state in the line under the chip.
  const DESKTOP = 'desktop';

  const members = $derived(kernelMembers(items));
  const layout = $derived(layoutKernel(members.length, narrow));
  const animated = $derived(motion.level === 'full');
  // Only a found update the user has not installed yet: one downloading, installed or failed is not "available".
  const desktopUpdate = $derived(updates.state.kind === 'available');
  const summary = $derived(kernelSummary(members));

  let hot = $state<string | null>(null);

  /** The module in focus: the chip's own hover reads in the status line but dims nothing. */
  const hotModule = $derived(hot === DESKTOP ? null : hot);
  const hotMember = $derived(members.find((member) => member.id === hot));

  const status = $derived.by(() => {
    if (hotMember) {
      return focusLine(hotMember.name, hotMember.state);
    }

    return hot === DESKTOP ? focusLine(FAMILY_NAMES.desktop, 'update') : summary;
  });

  /** One module's hover or focus: leaving one that is no longer the hot one (the pointer already moved on) changes nothing. */
  function heat(id: string, over: boolean): void {
    if (over) {
      hot = id;
    } else if (hot === id) {
      hot = null;
    }
  }

  /** Choosing a module brings its card in the list below into view. */
  function showCard(member: KernelMember): void {
    const card = document.getElementById(cardTitleId(member.id))?.closest('article');

    card?.scrollIntoView?.({ behavior: animated ? 'smooth' : 'auto', block: 'center' });
    card?.focus({ preventScroll: true });
  }
</script>

<div
  role="group"
  aria-label={summary}
  class="kernel relative mx-auto shrink-0"
  data-layout={layout.narrow ? 'narrow' : 'wide'}
  style:width="{layout.width}px"
  style:height="{layout.height}px"
>
  <span class="glow" style:left="{layout.chip.x - layout.chip.w / 2}px" style:top="{layout.chip.y - layout.chip.h / 2}px" style:width="{layout.chip.w * 2}px" style:height="{layout.chip.h * 2}px"></span>

  <KernelTraces {layout} {members} hot={hotModule} />
  {#if animated}<KernelBlips {layout} {members} hot={hotModule} />{/if}

  <KernelChip
    chip={layout.chip}
    pins={layout.pins}
    {animated}
    updateAvailable={desktopUpdate}
    onhover={desktopUpdate ? (over) => heat(DESKTOP, over) : undefined}
  />

  <KernelStatus rect={layout.status} max={layout.width} text={status} focused={hot !== null} />

  {#each layout.modules as slot (slot.index)}
    {@const member = members[slot.index]}

    <KernelModule
      {member}
      {slot}
      {animated}
      compact={layout.narrow}
      hot={hot === member.id}
      dimmed={hotModule !== null && hotModule !== member.id}
      onhot={(over) => heat(member.id, over)}
      onselect={() => showCard(member)}
    />
  {/each}
</div>

<style>
  /* The mix darkens each member's colour a little in the light theme so it reads on white. */
  .kernel {
    --kernel-mix: 100%;
    --kernel-glow: 0.17;
  }

  :global(:root[data-theme='light']) .kernel {
    --kernel-mix: 84%;
    --kernel-glow: 0.14;
  }

  .glow {
    position: absolute;
    border-radius: 50%;
    background: radial-gradient(closest-side, var(--accent-2), transparent);
    opacity: var(--kernel-glow);
    pointer-events: none;
  }
</style>

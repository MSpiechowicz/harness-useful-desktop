<script lang="ts">
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import IconButton from '$lib/components/IconButton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { formatCount } from '$lib/utils/format';
  import { keepStepFocus } from '$lib/utils/step-focus';

  import { PART_SEGMENTS_MAX } from './knowledge-format';

  interface Props {
    /** The zero-based part on screen. */
    index: number;
    total: number;
    onpick: (index: number) => void;
  }

  let { index, total, onpick }: Props = $props();

  /** How far Page Up and Page Down jump on the bar. */
  const PAGE_STEP = 10;

  let root: HTMLElement | undefined = $state();
  /** The part under a drag or a held key: the count follows it, and the text is read only when it is let go. */
  let preview = $state<number | null>(null);

  const shown = $derived(preview ?? index);
  const label = (number: number): string => t('knowledge.chunk.partOf', { number: formatCount(number + 1), total: formatCount(total) });
  const clamp = (part: number): number => Math.max(0, Math.min(total - 1, part));

  // An arrow that reaches the end turns off, so the keyboard moves on to the other one.
  async function step(by: number): Promise<void> {
    onpick(clamp(index + by));
    await keepStepFocus(root, '[data-part-step]');
  }

  function commit(): void {
    if (preview === null) {
      return;
    }

    const part = preview;

    preview = null;
    onpick(clamp(part));
  }

  function partAt(event: PointerEvent): number {
    const bar = event.currentTarget as HTMLElement;
    const box = bar.getBoundingClientRect();
    const thumb = parseFloat(getComputedStyle(bar).getPropertyValue('--part-thumb-w')) || 0;
    const fraction = (event.clientX - box.left - thumb / 2) / Math.max(1, box.width - thumb);

    return clamp(Math.round(fraction * (total - 1)));
  }

  function press(event: PointerEvent): void {
    // A right click opens a menu and its release may never reach the bar.
    if (event.button !== 0) {
      return;
    }

    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    preview = partAt(event);
  }

  function drag(event: PointerEvent): void {
    if (preview !== null) {
      preview = partAt(event);
    }
  }

  function keyTarget(key: string): number | null {
    const moves: Record<string, number> = {
      ArrowLeft: shown - 1,
      ArrowRight: shown + 1,
      PageUp: shown - PAGE_STEP,
      PageDown: shown + PAGE_STEP,
      Home: 0,
      End: total - 1,
    };

    return key in moves ? clamp(moves[key]) : null;
  }

  function onkeydown(event: KeyboardEvent): void {
    const next = event.altKey || event.ctrlKey || event.metaKey ? null : keyTarget(event.key);

    if (next === null) {
      return;
    }

    event.preventDefault();

    if (event.repeat) {
      preview = next;
    } else {
      preview = null;
      onpick(next);
    }
  }
</script>

<!-- Where this part sits in its section: one segment a part while they are wide enough to click, then one bar with a handle. -->
<div bind:this={root} class="flex min-h-control items-center gap-2">
  <span class="tabular shrink-0 text-xs text-fg-2">{label(shown)}</span>

  {#if total <= PART_SEGMENTS_MAX}
    <div class="flex min-w-0 flex-1 gap-[var(--part-segment-gap)]">
      {#each { length: total }, part (part)}
        <button
          type="button"
          aria-label={label(part)}
          aria-current={part === index ? 'true' : undefined}
          title={t('knowledge.chunk.part', { number: part + 1 })}
          class="group flex h-control min-w-0 flex-1 items-center"
          onclick={() => onpick(part)}
        >
          <span class="h-[var(--part-track-h)] w-full rounded-full transition-colors duration-[var(--dur-fast)] {part === index ? 'bg-accent-text' : 'bg-fg-3/40 group-hover:bg-fg-2'}"></span>
        </button>
      {/each}
    </div>
  {:else}
    <div
      role="slider"
      tabindex="0"
      aria-label={t('knowledge.chunk.parts')}
      aria-valuemin={1}
      aria-valuemax={total}
      aria-valuenow={shown + 1}
      aria-valuetext={label(shown)}
      class="group relative h-control min-w-0 flex-1 cursor-pointer touch-none rounded-sm"
      onpointerdown={press}
      onpointermove={drag}
      onpointerup={commit}
      onpointercancel={commit}
      onlostpointercapture={commit}
      {onkeydown}
      onkeyup={commit}
      onblur={commit}
    >
      <span class="absolute inset-x-0 top-1/2 h-[var(--part-track-h)] -translate-y-1/2 rounded-full bg-fg-3/40"></span>
      <span
        class="absolute top-1/2 h-[var(--part-track-h)] w-[var(--part-thumb-w)] -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent-text transition-[height] duration-[var(--dur-fast)] group-hover:h-[var(--part-thumb-hover-h)]"
        style:left="calc(var(--part-thumb-w) / 2 + (100% - var(--part-thumb-w)) * {shown / (total - 1)})"
      ></span>
    </div>
  {/if}

  <div class="flex shrink-0 items-center">
    <IconButton data-part-step icon={ChevronLeft} label={t(index === 0 ? 'knowledge.chunk.first' : 'knowledge.chunk.previous')} disabled={index === 0} onclick={() => void step(-1)} />
    <IconButton data-part-step icon={ChevronRight} label={t(index >= total - 1 ? 'knowledge.chunk.last' : 'knowledge.chunk.next')} disabled={index >= total - 1} onclick={() => void step(1)} />
  </div>
</div>

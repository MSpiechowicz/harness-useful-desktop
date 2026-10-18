<script lang="ts" generics="T extends string">
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  import { PILL_FRAME, PILL_RAISED, type StripOption } from './strip';

  interface Props {
    options: StripOption<T>[];
    value: T;
    /** `tabs` renders a tablist (used with a panel); `radio` renders a radiogroup. */
    kind: 'tabs' | 'radio';
    appearance: 'underline' | 'pill';
    label: string;
    /** Prefix for the generated tab/panel ids that tie tabs to their panel. */
    idBase: string;
    onchange?: (value: T) => void;
    class?: string;
  }

  let { options, value = $bindable(), kind, appearance, label, idBase, onchange, class: className }: Props = $props();

  let container: HTMLElement | undefined = $state();
  let buttons: (HTMLButtonElement | undefined)[] = $state([]);
  let box = $state({ x: 0, width: 0 });
  let measured = $state(false);
  let settled = $state(false);

  const activeIndex = $derived(options.findIndex((option) => option.value === value));

  function measure(): void {
    const element = buttons[activeIndex];
    if (!element) {
      return;
    }

    box = { x: element.offsetLeft, width: element.offsetWidth };
    measured = true;
  }

  // Re-measure when the selection or options change.
  $effect(() => {
    void [value, options];
    measure();
  });

  // Re-measure on resize; enable the slide animation only after the first placement.
  $effect(() => {
    if (!container || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(measure);
    observer.observe(container);
    const frame = requestAnimationFrame(() => (settled = true));

    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  });

  function select(option: StripOption<T>): void {
    if (option.disabled || option.value === value) {
      return;
    }

    value = option.value;
    onchange?.(option.value);
  }

  function onKeydown(event: KeyboardEvent, index: number): void {
    const next = rovingIndex(event.key, index, options.length, 'horizontal');
    if (next === null) {
      return;
    }

    event.preventDefault();
    select(options[next]);
    buttons[next]?.focus();
  }

  const INDICATOR: Record<'underline' | 'pill', string> = {
    underline: 'bottom-0 h-0.5 rounded-full bg-accent-fill',
    pill: `inset-y-[var(--segment-pad)] ${PILL_RAISED}`,
  };
</script>

<div
  bind:this={container}
  role={kind === 'tabs' ? 'tablist' : 'radiogroup'}
  aria-label={label}
  class={cx(
    'relative inline-flex items-center',
    appearance === 'pill' ? PILL_FRAME : 'gap-1 border-b border-line',
    className,
  )}
>
  <span
    aria-hidden="true"
    class={cx(
      'pointer-events-none absolute left-0',
      INDICATOR[appearance],
      measured ? 'opacity-100' : 'opacity-0',
      settled && 'transition-[transform,width] duration-[var(--dur-slow)] ease-[var(--ease-spring)]',
    )}
    style:width="{box.width}px"
    style:transform="translateX({box.x}px)"
  ></span>

  {#each options as option, index (option.value)}
    {@const selected = option.value === value}
    <button
      bind:this={buttons[index]}
      type="button"
      id="{idBase}-tab-{option.value}"
      role={kind === 'tabs' ? 'tab' : 'radio'}
      aria-selected={kind === 'tabs' ? selected : undefined}
      aria-checked={kind === 'radio' ? selected : undefined}
      aria-controls={kind === 'tabs' ? `${idBase}-panel` : undefined}
      tabindex={selected ? 0 : -1}
      disabled={option.disabled}
      onclick={() => select(option)}
      onkeydown={(event) => onKeydown(event, index)}
      class={cx(
        'relative z-10 inline-flex items-center gap-2 px-3.5 text-sm font-medium whitespace-nowrap transition-colors duration-[var(--dur-fast)] disabled:cursor-not-allowed disabled:opacity-50',
        // Underline tabs sit in a strip that clips anything outside them, so their focus ring is drawn inside the tab,
        // far enough in to leave a clear gap above the selected tab's underline instead of touching it.
        appearance === 'pill' ? 'h-full rounded-sm' : 'h-control rounded-md focus-visible:outline-offset-[-6px]',
        selected ? 'text-fg' : 'text-fg-2 hover:text-fg',
      )}
    >
      {#if option.icon}
        {@const Icon = option.icon}
        <Icon size={16} strokeWidth={1.8} aria-hidden="true" />
      {/if}
      {option.label}
      {#if option.count !== undefined}
        <span class="tabular inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-surface-3 px-1.5 text-xs leading-none text-fg-2">{option.count}</span>
      {/if}
    </button>
  {/each}
</div>

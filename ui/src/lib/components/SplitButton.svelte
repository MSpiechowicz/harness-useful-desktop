<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  import Button from './Button.svelte';
  import { ICON, type IconComponent } from './icon';
  import MenuItem from './MenuItem.svelte';
  import type { SplitItem } from './split';

  interface Props {
    /** The label of the main action. */
    label: string;
    icon?: IconComponent;
    variant?: 'primary' | 'secondary';
    disabled?: boolean;
    loading?: boolean;
    /** Accessible name of the chevron that opens the other ways to do it. */
    menuLabel: string;
    items: SplitItem[];
    onclick: () => void;
    class?: string;
  }

  let { label, icon: Icon, variant = 'secondary', disabled = false, loading = false, menuLabel, items, onclick, class: className }: Props = $props();

  let root: HTMLElement | undefined = $state();
  let menu: HTMLElement | undefined = $state();
  let open = $state(false);

  // The menu sits inside the panel around it instead of opening a popover of its own: a popover inside a popover is clipped by it.
  $effect(() => {
    if (open) {
      queueMicrotask(() => menu?.querySelector<HTMLElement>('[data-menu-item]:not(:disabled)')?.focus());
    }
  });

  // Not being usable closes the menu, so it never stays open over a button that went grey.
  $effect(() => {
    if (disabled || loading) {
      open = false;
    }
  });

  function close(returnFocus: boolean): void {
    open = false;

    if (returnFocus) {
      root?.querySelector<HTMLElement>('[data-split-chevron]')?.focus();
    }
  }

  function onPointerDown(event: PointerEvent): void {
    if (open && root && event.target instanceof Node && !root.contains(event.target)) {
      open = false;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (!open) {
      return;
    }

    // Escape and the arrows belong to the menu while it is open, not to the panel around it.
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close(true);

      return;
    }

    const entries = Array.from(menu?.querySelectorAll<HTMLElement>('[data-menu-item]:not(:disabled)') ?? []);
    const next = entries.length === 0 ? null : rovingIndex(event.key, entries.indexOf(document.activeElement as HTMLElement), entries.length, 'vertical');

    if (next !== null) {
      event.preventDefault();
      event.stopPropagation();
      entries[next].focus();
    }
  }

  function choose(item: SplitItem): void {
    close(false);
    item.onselect();
  }
</script>

<svelte:window onpointerdown={onPointerDown} />

<div bind:this={root} class={cx('relative inline-flex shrink-0', className)} onkeydown={onKeydown} role="presentation">
  <Button {variant} {disabled} {loading} {onclick} class="rounded-r-none!">
    {#snippet leading()}{#if Icon}<Icon size={ICON.sm} aria-hidden="true" />{/if}{/snippet}
    {label}
  </Button>

  <Button
    {variant}
    bare
    disabled={disabled || loading}
    data-split-chevron
    aria-label={menuLabel}
    aria-haspopup="menu"
    aria-expanded={open}
    class={cx('rounded-l-none! px-2!', variant === 'primary' ? 'border-l border-l-on-accent/25' : '-ml-px')}
    onclick={() => (open = !open)}
  >
    <ChevronDown size={ICON.sm} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-180')} />
  </Button>

  {#if open}
    <div bind:this={menu} role="menu" aria-label={menuLabel} class="absolute top-full right-0 z-10 mt-1 flex w-72 max-w-[calc(100vw-2rem)] flex-col rounded-lg border border-line-strong bg-glass p-1 shadow-pop backdrop-blur-xl">
      {#each items as item (item.id)}
        <MenuItem icon={item.icon} disabled={item.disabled} class="items-start" onclick={() => choose(item)}>
          <span class="flex min-w-0 flex-col">
            <span>{item.label}</span>
            {#if item.description}<span class="text-xs text-fg-3">{item.description}</span>{/if}
          </span>
        </MenuItem>
      {/each}
    </div>
  {/if}
</div>

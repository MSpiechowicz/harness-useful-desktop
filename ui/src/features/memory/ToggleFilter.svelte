<script lang="ts">
  import { ICON, type IconComponent } from '$lib/components/icon';
  import { cx } from '$lib/utils/cx';

  interface Props {
    icon: IconComponent;
    label: string;
    /** Whether the list is narrowed by it. */
    on: boolean;
    onchange: (on: boolean) => void;
  }

  let { icon: Icon, label, on, onchange }: Props = $props();
</script>

<!-- A switch that narrows the list. It looks like the filter menus beside it, accent while it narrows the list. -->
<button
  type="button"
  aria-pressed={on}
  class={cx(
    'inline-flex h-control items-center gap-2 rounded-md border pr-3 pl-3 text-base font-medium whitespace-nowrap transition-[transform,background-color,border-color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] active:scale-[0.97]',
    on ? 'border-accent/30 bg-accent-wash text-accent-text' : 'border-line-strong bg-surface-2 text-fg shadow-raised hover:bg-surface-3',
  )}
  onclick={() => onchange(!on)}
>
  <Icon size={ICON.sm} class={on ? undefined : 'text-fg-3'} aria-hidden="true" />
  {label}
</button>

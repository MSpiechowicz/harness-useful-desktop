<script lang="ts">
  import EyeOff from '@lucide/svelte/icons/eye-off';

  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** How many team notes are hidden on this computer. */
    count: number;
    /** Whether the list shows those notes. */
    on: boolean;
    onchange: (on: boolean) => void;
  }

  let { count, on, onchange }: Props = $props();
</script>

<!-- A switch between the notes agents see and the team notes hidden from them here. It looks like the filter menus beside it, accent while it narrows the list. -->
<button
  type="button"
  aria-pressed={on}
  class={cx(
    'inline-flex h-control items-center gap-2 rounded-md border pr-3 pl-3 text-base font-medium whitespace-nowrap transition-[transform,background-color,border-color] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] active:scale-[0.97]',
    on ? 'border-accent/30 bg-accent-wash text-accent-text' : 'border-line-strong bg-surface-2 text-fg shadow-raised hover:bg-surface-3',
  )}
  onclick={() => onchange(!on)}
>
  <EyeOff size={ICON.sm} class={on ? undefined : 'text-fg-3'} aria-hidden="true" />
  {t('memory.hidden.filter', { count })}
</button>

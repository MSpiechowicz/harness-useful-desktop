<script lang="ts">
  import Switch from '$lib/components/Switch.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    title: string;
    description: string;
    /** A name inside the description to set off, the repository. */
    strong?: string;
    checked: boolean;
    /** Accessible name of the switch. */
    label: string;
    disabled?: boolean;
    /** A wider setting is off, so this one does nothing: the row is dimmed and its switch cannot be used. */
    dim?: boolean;
    onchange: (checked: boolean) => void;
  }

  let { title, description, strong, checked, label, disabled = false, dim = false, onchange }: Props = $props();

  const at = $derived(strong ? description.indexOf(strong) : -1);
</script>

<!-- One setting of a kind: what it is, what it does now, and its switch in the middle of the row. -->
<div class="flex items-center gap-3 border-t border-line px-3 py-2.5">
  <div class={cx('flex min-w-0 flex-1 flex-col gap-0.5 transition-opacity duration-[var(--dur-fast)]', dim && 'opacity-45')}>
    <p class="text-sm font-medium text-fg">{title}</p>
    <p class="text-xs text-fg-3">
      {#if strong && at >= 0}
        {description.slice(0, at)}<b class="font-medium text-fg-2">{strong}</b>{description.slice(at + strong.length)}
      {:else}
        {description}
      {/if}
    </p>
  </div>

  <Switch {label} {disabled} bind:checked={() => checked, onchange} />
</div>

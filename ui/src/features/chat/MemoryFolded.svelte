<script lang="ts">
  import type { Snippet } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The content is long: it is cut short with a fade, and "Show all" opens it. Short content is shown as it is. */
    long: boolean;
    children: Snippet;
  }

  let { long, children }: Props = $props();

  let open = $state(false);
  const folded = $derived(long && !open);
</script>

<div class="flex flex-col items-start gap-1.5">
  <div class={cx('relative w-full', folded && 'max-h-44 overflow-hidden')} data-folded={folded || undefined}>
    {@render children()}

    {#if folded}
      <div class="pointer-events-none absolute inset-x-0 bottom-0 h-12 bg-gradient-to-t from-surface-1 to-transparent" aria-hidden="true"></div>
    {/if}
  </div>

  {#if long}
    <button type="button" aria-expanded={open} onclick={() => (open = !open)} class="rounded-sm text-sm text-accent-text transition-colors duration-[var(--dur-fast)] hover:text-fg">
      {open ? t('memory.guards.waiting.showLess') : t('memory.guards.waiting.showAll')}
    </button>
  {/if}
</div>

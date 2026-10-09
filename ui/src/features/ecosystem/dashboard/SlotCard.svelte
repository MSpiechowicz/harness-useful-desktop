<script lang="ts" generics="T">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { rise } from '$lib/design/motion';
  import type { QuerySlot } from '$lib/stores/dashboard.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  interface Props {
    title: string;
    /** One line under the title: the range or the source of the numbers. */
    subtitle?: string;
    slot: QuerySlot<T>;
    onretry: () => void;
    /** Position in the panel; cards enter one after another. */
    index?: number;
    /** Extra controls in the header, e.g. a chart switch. */
    actions?: Snippet;
    children: Snippet<[T]>;
    class?: string;
  }

  let { title, subtitle, slot, onretry, index = 0, actions, children, class: className }: Props = $props();

  const titleId = uid('slot-card');
</script>

<section
  aria-labelledby={titleId}
  aria-busy={slot.status === 'loading'}
  in:rise={{ y: 14, delay: index * 60, duration: 420 }}
  class={cx('flex min-w-0 flex-col gap-4 rounded-lg border border-line bg-surface-1 p-5 shadow-raised', className)}
>
  <header class="flex items-start justify-between gap-3">
    <div class="flex min-w-0 flex-col">
      <h3 id={titleId} class="text-base font-semibold text-fg">{title}</h3>
      {#if subtitle}<p class="text-sm text-fg-3">{subtitle}</p>{/if}
    </div>
    {#if actions}<div class="shrink-0">{@render actions()}</div>{/if}
  </header>

  {#if slot.data !== null}
    <!-- Reloading keeps the old numbers on screen, dimmed, instead of flashing a skeleton. -->
    <div class={cx('transition-opacity duration-[var(--dur-base)]', slot.stale && 'opacity-60')}>
      {@render children(slot.data)}
    </div>
  {:else if slot.status === 'error'}
    <div class="flex items-start gap-3 rounded-md border border-line bg-danger-wash p-3.5 text-sm" role="alert">
      <CircleAlert class="mt-0.5 shrink-0 text-danger" size={16} aria-hidden="true" />
      <div class="flex min-w-0 flex-1 flex-col items-start gap-2">
        <p class="text-fg-2">{slot.error}</p>
        <Button size="sm" onclick={onretry}>Try again</Button>
      </div>
    </div>
  {:else}
    <div class="flex flex-col gap-3" role="status" aria-label="Loading {title.toLowerCase()}">
      <Skeleton height="1.25rem" />
      <Skeleton height="1.25rem" width="85%" />
      <Skeleton height="1.25rem" width="70%" />
    </div>
  {/if}
</section>

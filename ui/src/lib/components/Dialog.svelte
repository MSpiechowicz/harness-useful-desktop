<script lang="ts">
  import type { Snippet } from 'svelte';
  import X from '@lucide/svelte/icons/x';

  import { uid } from '$lib/utils/ids';
  import { cx } from '$lib/utils/cx';

  import IconButton from './IconButton.svelte';
  import Overlay from './Overlay.svelte';

  type Size = 'sm' | 'md' | 'lg';

  interface Props {
    open?: boolean;
    title: string;
    description?: string;
    size?: Size;
    dismissible?: boolean;
    onclose?: () => void;
    footer?: Snippet;
    children: Snippet;
  }

  let { open = $bindable(false), title, description, size = 'md', dismissible = true, onclose, footer, children }: Props = $props();

  const SIZES: Record<Size, string> = { sm: 'w-[24rem]', md: 'w-[32rem]', lg: 'w-[44rem]' };

  const titleId = uid('dialog-title');
  const descriptionId = uid('dialog-description');

  function close(): void {
    open = false;
    onclose?.();
  }
</script>

<Overlay {open} labelledby={titleId} describedby={description ? descriptionId : undefined} {dismissible} onclose={close}>
  <div class={cx('flex max-h-[85vh] flex-col rounded-xl border border-line-strong bg-glass shadow-pop backdrop-blur-xl', SIZES[size])}>
    <header class="flex items-start justify-between gap-4 px-6 pt-5">
      <div class="flex flex-col gap-1">
        <h2 id={titleId} class="text-xl text-fg">{title}</h2>
        {#if description}
          <p id={descriptionId} class="text-base text-fg-2">{description}</p>
        {/if}
      </div>

      {#if dismissible}
        <IconButton icon={X} label="Close dialog" size="sm" data-dismiss onclick={close} />
      {/if}
    </header>

    <div class="overflow-y-auto px-6 py-5">
      {@render children()}
    </div>

    {#if footer}
      <footer class="flex justify-end gap-2 border-t border-line px-6 py-4">
        {@render footer()}
      </footer>
    {/if}
  </div>
</Overlay>

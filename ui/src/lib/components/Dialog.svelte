<script lang="ts">
  import type { Snippet } from 'svelte';
  import X from '@lucide/svelte/icons/x';

  import { t } from '$lib/i18n/i18n.svelte';
  import { uid } from '$lib/utils/ids';
  import { cx } from '$lib/utils/cx';

  import IconButton from './IconButton.svelte';
  import Overlay from './Overlay.svelte';

  type Size = 'sm' | 'md' | 'lg' | 'xl';

  interface Props {
    open?: boolean;
    title: string;
    /** What the dialog acts on, e.g. the session being deleted: a row between hairlines under the text. */
    subject?: Snippet;
    description?: string;
    size?: Size;
    dismissible?: boolean;
    /** Selector of the control that takes the focus first, e.g. the safe choice of a confirmation. */
    initialFocus?: string;
    onclose?: () => void;
    footer?: Snippet;
    /** The body; a confirmation has none, only its title, description and buttons. */
    children?: Snippet;
  }

  let { open = $bindable(false), title, subject, description, size = 'md', dismissible = true, initialFocus, onclose, footer, children }: Props = $props();

  const SIZES: Record<Size, string> = { sm: 'w-[24rem]', md: 'w-[32rem]', lg: 'w-[44rem]', xl: 'w-[min(62rem,calc(100vw-2rem))]' };

  const titleId = uid('dialog-title');
  const descriptionId = uid('dialog-description');

  function close(): void {
    open = false;
    onclose?.();
  }
</script>

<Overlay {open} labelledby={titleId} describedby={description ? descriptionId : undefined} {dismissible} {initialFocus} onclose={close}>
  <div class={cx('flex max-h-[85vh] flex-col rounded-xl border border-line-strong bg-glass shadow-pop backdrop-blur-xl', SIZES[size])}>
    <!-- The close button is centred on the title line; its negative margins keep the 36px control from making the line
         taller or pushing the glyph in from the edge. The title's letters start 16px below the top edge (9px of
         padding plus the leading above them), the same as the room under the footer's buttons. -->
    <header class={cx('flex flex-col gap-1 px-6 pt-[9px]', subject ? 'pb-4' : !children && 'pb-5')}>
      <div class="flex items-center justify-between gap-4">
        <h2 id={titleId} class="text-xl text-fg">{title}</h2>
        {#if dismissible}
          <IconButton icon={X} label={t('components.dialog.close')} class="-my-2 -mr-1.5 shrink-0" data-dismiss onclick={close} />
        {/if}
      </div>

      {#if description}
        <p id={descriptionId} class="text-base text-fg-2">{description}</p>
      {/if}
    </header>

    {#if subject}
      <div class="border-t border-line px-6 py-3.5">
        {@render subject()}
      </div>
    {/if}

    {#if children}
      <div class="overflow-y-auto px-6 py-5">
        {@render children()}
      </div>
    {/if}

    {#if footer}
      <footer class="flex justify-end gap-2 border-t border-line px-6 py-[16px]">
        {@render footer()}
      </footer>
    {/if}
  </div>
</Overlay>

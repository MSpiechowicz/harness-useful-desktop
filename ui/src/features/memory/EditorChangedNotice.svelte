<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The item is gone, not only changed: there is nothing to reload. */
    gone: boolean;
    /** What to say when the item changed, and when it is gone. */
    changedText: string;
    goneText: string;
    /** The item is being read again. */
    reloading: boolean;
    onreload: () => void;
    onclose: () => void;
  }

  let { gone, changedText, goneText, reloading, onreload, onclose }: Props = $props();

  const look = NOTICE_LOOKS.warning;
</script>

<!-- What the save found: the note or script is not the version the editor opened. Nothing was written, and the edits stay until the user chooses to reload. -->
<div class={cx('flex flex-col items-start gap-3 rounded-md p-3', look.box)} role="alert">
  <div class="flex min-w-0 items-start gap-3">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <p class="min-w-0 text-sm break-words text-fg-2">{gone ? goneText : changedText}</p>
  </div>

  {#if gone}
    <Button onclick={onclose}>{t('memory.editor.gone.close')}</Button>
  {:else}
    <Button variant="primary" loading={reloading} onclick={onreload}>{t('memory.editor.reload')}</Button>
  {/if}
</div>

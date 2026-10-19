<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The script is gone, not only changed: there is nothing to reload. */
    gone: boolean;
    /** The script is being read again. */
    reloading: boolean;
    onreload: () => void;
    onclose: () => void;
  }

  let { gone, reloading, onreload, onclose }: Props = $props();

  const look = NOTICE_LOOKS.warning;
</script>

<!-- What the save found: the script is not the version the editor opened. Nothing was written, and the edits stay until the user chooses to reload. -->
<div class={cx('flex flex-col items-start gap-3 rounded-md p-3', look.box)} role="alert">
  <div class="flex min-w-0 items-start gap-3">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <p class="min-w-0 text-sm break-words text-fg-2">{t(gone ? 'memory.editor.script.gone' : 'memory.editor.script.changed')}</p>
  </div>

  {#if gone}
    <Button onclick={onclose}>{t('memory.editor.script.gone.close')}</Button>
  {:else}
    <Button variant="primary" loading={reloading} onclick={onreload}>{t('memory.editor.script.changed.reload')}</Button>
  {/if}
</div>

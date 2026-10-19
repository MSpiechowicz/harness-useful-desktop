<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    title: string;
    body: string;
    /** The replace is being written. */
    replacing: boolean;
    onopen: () => void;
    /** Absent when replacing is not possible, e.g. renaming a note onto another one would leave the edited note behind. */
    onreplace?: () => void;
  }

  let { title, body, replacing, onopen, onreplace }: Props = $props();

  const look = NOTICE_LOOKS.warning;
</script>

<!-- What the save found: an item with this title or name is already kept. Nothing was written yet, so the user chooses. -->
<div class={cx('flex flex-col items-start gap-3 rounded-md p-3', look.box)} role="alert">
  <div class="flex min-w-0 items-start gap-3">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <div class="flex min-w-0 flex-col gap-0.5">
      <strong class="text-sm font-medium text-fg">{title}</strong>
      <span class="text-sm break-words text-fg-2">{body}</span>
    </div>
  </div>

  <div class="flex flex-wrap items-center gap-2">
    <Button onclick={onopen}>{t('memory.editor.exists.open')}</Button>
    {#if onreplace}
      <Button variant="primary" loading={replacing} onclick={onreplace}>{t('memory.editor.exists.replace')}</Button>
    {/if}
  </div>
</div>

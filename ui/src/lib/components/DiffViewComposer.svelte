<script lang="ts">
  import { untrack } from 'svelte';

  import Button from './Button.svelte';
  import { autofocus } from '$lib/a11y/autofocus';
  import { t } from '$lib/i18n/i18n.svelte';
  import { uid } from '$lib/utils/ids';

  interface Props {
    /** What the comment is about, e.g. "Lines 3–5 (new)". */
    heading: string;
    initial?: string;
    saveLabel?: string;
    /** Longest allowed body in bytes. */
    maxBytes: number;
    onsave: (body: string) => void;
    oncancel: () => void;
  }

  let { heading, initial = '', saveLabel, maxBytes, onsave, oncancel }: Props = $props();

  const fieldId = uid('comment');
  let body = $state(untrack(() => initial));

  const text = $derived(body.trim());
  const tooLong = $derived(new TextEncoder().encode(body).length > maxBytes);

  function save(): void {
    if (text !== '' && !tooLong) {
      onsave(text);
    }
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      oncancel();
    } else if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      save();
    }
  }
</script>

<div class="flex flex-col gap-2 rounded-md border border-line-strong bg-surface-1 p-3 shadow-raised">
  <label for={fieldId} class="text-xs font-medium text-fg-2">{heading}</label>
  <textarea
    id={fieldId}
    bind:value={body}
    use:autofocus
    {onkeydown}
    rows="3"
    placeholder={t('components.diff.placeholder')}
    class="w-full resize-none rounded-sm border border-line bg-surface-2 px-3 py-2 text-base text-fg outline-none placeholder:text-fg-3 focus:border-focus"
  ></textarea>

  <div class="flex items-center justify-end gap-2">
    {#if tooLong}<span class="mr-auto text-xs text-danger" role="alert">{t('components.diff.tooLong', { size: maxBytes / 1024 })}</span>{/if}
    <Button size="sm" variant="ghost" onclick={oncancel}>{t('components.diff.cancel')}</Button>
    <Button size="sm" variant="primary" disabled={text === '' || tooLong} onclick={save}>{saveLabel ?? t('components.diff.comment')}</Button>
  </div>
</div>

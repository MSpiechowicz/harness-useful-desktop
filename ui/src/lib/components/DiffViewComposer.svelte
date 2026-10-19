<script lang="ts">
  import { untrack } from 'svelte';

  import Badge from './Badge.svelte';
  import Button from './Button.svelte';
  import Kbd from './Kbd.svelte';
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

<!-- A card no wider than a comfortable line, not a bar across the whole diff. The selected lines are already highlighted above
     it, so it only names them; the footer says how to send and cancel. -->
<div class="@container flex max-w-(--comment-card) flex-col rounded-lg border border-line-strong bg-surface-1 shadow-raised">
  <div class="px-4 pt-4">
    <label for={fieldId} class="inline-flex"><Badge tone="accent" class="h-control rounded-md! px-2.5 text-sm!">{heading}</Badge></label>
  </div>

  <div class="px-4 py-3">
    <textarea
      id={fieldId}
      bind:value={body}
      use:autofocus
      {onkeydown}
      rows="6"
      placeholder={t('components.diff.placeholder')}
      class="w-full resize-none rounded-md border border-line bg-surface-2 px-4 py-3 text-base leading-relaxed text-fg outline-none placeholder:text-fg-3 focus:border-focus"
    ></textarea>
  </div>

  <div class="flex items-center gap-2 border-t border-line px-4 py-3">
    {#if tooLong}
      <span class="text-xs text-danger" role="alert">{t('components.diff.tooLong', { size: maxBytes / 1024 })}</span>
    {:else}
      <span class="hidden items-center gap-1.5 text-xs text-fg-3 @md:inline-flex">
        <Kbd keys="mod+enter" small />{t('components.diff.hintSend')}<span aria-hidden="true">·</span><Kbd keys="escape" small />{t('components.diff.hintCancel')}
      </span>
    {/if}
    <span class="flex-1"></span>
    <Button variant="ghost" onclick={oncancel}>{t('components.diff.cancel')}</Button>
    <Button variant="primary" disabled={text === '' || tooLong} onclick={save}>{saveLabel ?? t('components.diff.comment')}</Button>
  </div>
</div>

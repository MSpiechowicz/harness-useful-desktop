<script lang="ts">
  import type { Snippet } from 'svelte';

  import Dialog from '$lib/components/Dialog.svelte';
  import DocEditorFooter from '$lib/components/docs/DocEditorFooter.svelte';
  import DocFormTail from '$lib/components/docs/DocFormTail.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    open: boolean;
    title: string;
    description: string;
    /** Selector of the field the focus starts on. */
    initialFocus: string;
    /** Something in the form keeps it from being saved. */
    blocked: boolean;
    /** The save is being written. */
    saving: boolean;
    saveLabel: string;
    /** Why the last save failed, as the backend said it. */
    failure?: string | null;
    onsubmit: () => void;
    children: Snippet;
  }

  let { open = $bindable(false), title, description, initialFocus, saveLabel, saving, blocked, failure = null, onsubmit, children }: Props = $props();
</script>

<Dialog bind:open size="lg" {title} {description} {initialFocus}>
  <form
    class="flex flex-col gap-5"
    onsubmit={(event) => {
      event.preventDefault();
      onsubmit();
    }}
  >
    {@render children()}

    <DocFormTail {failure} {saveLabel} />
  </form>

  {#snippet footer()}
    <DocEditorFooter cancelLabel={t('memory.editor.cancel')} {saveLabel} {saving} {blocked} oncancel={() => (open = false)} onsave={onsubmit} />
  {/snippet}
</Dialog>

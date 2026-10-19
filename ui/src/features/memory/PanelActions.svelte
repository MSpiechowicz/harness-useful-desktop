<script lang="ts">
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** What the edit button says: "Edit". */
    editLabel: string;
    /** Opens the editor on the item. Absent until the item is read, so the button waits. */
    onedit: (() => void) | undefined;
    /** What the delete button says: "Delete". */
    deleteLabel: string;
    ondelete: () => void;
    /** What else can be done with the item, above the delete button. */
    children?: Snippet;
  }

  let { editLabel, onedit, deleteLabel, ondelete, children }: Props = $props();
</script>

<SectionCard flat heading={t('memory.panel.actions')}>
  <div class="flex flex-col items-start gap-3">
    {@render children?.()}

    <div class="flex flex-wrap items-center gap-2">
      <Button disabled={!onedit} onclick={onedit}>
        {#snippet leading()}<Pencil size={ICON.sm} aria-hidden="true" />{/snippet}
        {editLabel}
      </Button>

      <Button variant="dangerQuiet" onclick={ondelete}>
        {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
        {deleteLabel}
      </Button>
    </div>
  </div>
</SectionCard>

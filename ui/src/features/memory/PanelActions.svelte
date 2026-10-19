<script lang="ts">
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
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
    /** The item cannot be changed here: its Edit and Delete buttons are left out. */
    readOnly?: boolean;
    /** Hides the item on this computer (a team note), or shows it again when `hidden`. Absent for an item that cannot be hidden. */
    onhide?: () => void;
    hidden?: boolean;
    /** What else can be done with the item, above the delete button. */
    children?: Snippet;
  }

  let { editLabel, onedit, deleteLabel, ondelete, readOnly = false, onhide, hidden = false, children }: Props = $props();
</script>

<SectionCard flat heading={t('memory.panel.actions')}>
  <div class="flex flex-col items-start gap-3">
    {@render children?.()}

    <div class="flex flex-wrap items-center gap-2">
      {#if !readOnly}
        <Button disabled={!onedit} onclick={onedit}>
          {#snippet leading()}<Pencil size={ICON.sm} aria-hidden="true" />{/snippet}
          {editLabel}
        </Button>
      {/if}

      {#if onhide}
        <Button onclick={onhide}>
          {#snippet leading()}
            {#if hidden}<Eye size={ICON.sm} aria-hidden="true" />{:else}<EyeOff size={ICON.sm} aria-hidden="true" />{/if}
          {/snippet}
          {t(hidden ? 'memory.hidden.unhide' : 'memory.hidden.hide')}
        </Button>
      {/if}

      {#if !readOnly}
        <Button variant="dangerQuiet" onclick={ondelete}>
          {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
          {deleteLabel}
        </Button>
      {/if}
    </div>
  </div>
</SectionCard>

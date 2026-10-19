<script lang="ts">
  import { untrack } from 'svelte';

  import { IpcFailure } from '$lib/ipc/transport';

  import ConfirmDialog from '../ConfirmDialog.svelte';
  import type { IconComponent } from '../icon';

  interface Props {
    open: boolean;
    /** How many documents the list holds: the number the question names. */
    count: number;
    icon: IconComponent;
    /** The question and the name of what goes, for a count. */
    title: (count: number) => string;
    subject: (count: number) => string;
    description: string;
    confirmLabel: string;
    cancelLabel: string;
    /** Says what could not be done, before the reason the backend gave. */
    failed: string;
    /** Deletes them. Throws what the backend says, for the dialog to show. */
    remove: () => Promise<unknown>;
    /** They are gone: whatever was open on one lets go of it. */
    onremoved: () => void;
  }

  let { open = $bindable(false), count, icon, title, subject, description, confirmLabel, cancelLabel, failed, remove, onremoved }: Props = $props();

  // The number the question was asked with, kept while it fades out so the dialog shows the same words.
  let asked = $state(0);

  // Read when it opens only: the list shrinks while the documents are deleted, and the question must not change under the user.
  $effect(() => {
    if (open) {
      asked = untrack(() => count);
    }
  });

  async function confirm(): Promise<void> {
    try {
      await remove();
    } catch (error) {
      throw new Error(`${failed}: ${IpcFailure.from(error).message}`);
    }

    onremoved();
  }
</script>

<ConfirmDialog bind:open title={title(asked)} {description} subject={{ icon, name: subject(asked) }} {confirmLabel} {cancelLabel} onconfirm={confirm} />

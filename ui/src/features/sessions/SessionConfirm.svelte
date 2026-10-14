<script lang="ts" module>
  import type { ConfirmSubject } from '$lib/components/ConfirmDialog.svelte';

  export interface ConfirmRequest {
    title: string;
    description: string;
    /** What is deleted. */
    subject?: ConfirmSubject;
    /** The destructive action's verb: "Delete session". */
    confirmLabel: string;
  }
</script>

<script lang="ts">
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

  let request = $state<ConfirmRequest>({ title: '', description: '', confirmLabel: '' });
  let open = $state(false);
  let settle: ((confirmed: boolean) => void) | null = null;

  /** Opens the dialog; resolves `true` when the user confirms, `false` when they cancel or dismiss it. */
  export function ask(next: ConfirmRequest): Promise<boolean> {
    answer(false);
    request = next;
    open = true;

    return new Promise((resolve) => {
      settle = resolve;
    });
  }

  function answer(confirmed: boolean): void {
    settle?.(confirmed);
    settle = null;
  }

  // Closing the dialog any other way than confirming is a "no".
  $effect(() => {
    if (!open) {
      answer(false);
    }
  });
</script>

<ConfirmDialog bind:open title={request.title} description={request.description} subject={request.subject} confirmLabel={request.confirmLabel} onconfirm={() => answer(true)} />

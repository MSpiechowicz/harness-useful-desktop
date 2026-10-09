<script lang="ts">
  import Button from './Button.svelte';
  import Dialog from './Dialog.svelte';

  interface Props {
    open?: boolean;
    title: string;
    description: string;
    /** The destructive action's verb: "Stop chat". */
    confirmLabel: string;
    cancelLabel?: string;
    onconfirm: () => void;
  }

  let { open = $bindable(false), title, description, confirmLabel, cancelLabel = 'Keep it', onconfirm }: Props = $props();

  function confirm(): void {
    open = false;
    onconfirm();
  }
</script>

<!-- Destructive actions are confirmed in the danger tone; the safe choice comes first and holds the focus. -->
<Dialog bind:open {title} {description} size="sm">
  {#snippet children()}{/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{cancelLabel}</Button>
    <Button variant="danger" onclick={confirm}>{confirmLabel}</Button>
  {/snippet}
</Dialog>

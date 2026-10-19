<script lang="ts">
  import Button from '../Button.svelte';

  interface Props {
    cancelLabel: string;
    saveLabel: string;
    /** The save is being written. */
    saving: boolean;
    /** Something in the form keeps it from being saved. */
    blocked: boolean;
    oncancel: () => void;
    onsave: () => void;
  }

  let { cancelLabel, saveLabel, saving, blocked, oncancel, onsave }: Props = $props();
</script>

<Button variant="ghost" onclick={oncancel}>{cancelLabel}</Button>
<Button variant="primary" loading={saving} disabled={blocked} onclick={onsave}>{saveLabel}</Button>

<script lang="ts">
  interface Props {
    /** Why the save failed, as the backend said it. */
    failure: string | null;
    /** The label of the hidden submit button. */
    saveLabel: string;
  }

  let { failure, saveLabel }: Props = $props();
</script>

{#if failure}<p class="text-sm text-danger" role="alert">{failure}</p>{/if}

<!-- Enter in a field saves through this hidden button. -->
<button type="submit" class="hidden" tabindex="-1" aria-hidden="true">{saveLabel}</button>

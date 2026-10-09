<script lang="ts">
  import ErrorBoundary from '$lib/components/ErrorBoundary.svelte';

  interface Props {
    variant?: 'view' | 'screen' | 'silent';
    /** Throws while rendering as long as this returns true. */
    broken: () => boolean;
  }

  let { variant = 'view', broken }: Props = $props();

  function content(): string {
    if (broken()) {
      throw new Error('each_key_duplicate: Keyed each block has duplicate key');
    }

    return 'Page content';
  }
</script>

<ErrorBoundary area="The Ecosystem page" {variant}>
  <p>{content()}</p>
</ErrorBoundary>
<p>Outside the boundary</p>

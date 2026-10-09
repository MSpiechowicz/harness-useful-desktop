<script lang="ts">
  import { fade } from 'svelte/transition';

  import { rise, scaled } from '$lib/design/motion';
  import { nav } from '$lib/stores/nav.svelte';
  import ConfigViewer from '../config/ConfigViewer.svelte';
  import ExtensionsViewer from '../extensions/ExtensionsViewer.svelte';
  import SettingsView from '../settings/SettingsView.svelte';

  import PlaceholderView from './PlaceholderView.svelte';
  import { viewMeta } from './views';

  const meta = $derived(viewMeta(nav.current));
</script>

<!-- Stacked grid cell so the outgoing and incoming views crossfade in place. -->
<div class="grid size-full min-h-0 *:col-start-1 *:row-start-1">
  {#key nav.current}
    <div
      class="size-full min-h-0 overflow-auto"
      in:rise={{ y: 10 * nav.direction, delay: 80, duration: 320 }}
      out:fade={{ duration: scaled(120) }}
    >
      <h1 class="sr-only" aria-live="polite">{meta.label}</h1>

      {#if nav.current === 'settings'}
        <SettingsView />
      {:else if nav.current === 'config'}
        <ConfigViewer />
      {:else if nav.current === 'extensions'}
        <ExtensionsViewer />
      {:else}
        <PlaceholderView view={meta} />
      {/if}
    </div>
  {/key}
</div>

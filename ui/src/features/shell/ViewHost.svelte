<script lang="ts">
  import { fade } from 'svelte/transition';

  import ErrorBoundary from '$lib/components/ErrorBoundary.svelte';
  import ErrorPanel from '$lib/components/ErrorPanel.svelte';
  import { rise, scaled } from '$lib/design/motion';
  import { nav } from '$lib/stores/nav.svelte';
  import { lazyPage } from '$lib/utils/lazy-page';
  import ConfigViewer from '../config/ConfigViewer.svelte';
  import EcosystemView from '../ecosystem/EcosystemView.svelte';
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
      <!-- Each view's own header carries the h1; this only announces the move to screen readers. -->
      <p class="sr-only" role="status">{meta.label}</p>

      <!-- A page that throws shows "Something went wrong" in its place; switching pages remounts (and retries) it.
           A lazy page whose code fails to load gets the same panel, with only a reload to fetch it again. -->
      <ErrorBoundary area="The {meta.label} page">
        {#if nav.current === 'chat'}
          <!-- Split out of the first load: Markdown, the sanitizer and the chat UI arrive while the intro plays. -->
          {#await lazyPage(() => import('../chat/ChatView.svelte')) then { default: ChatView }}
            <ChatView />
          {:catch error}
            <ErrorPanel area="The {meta.label} page" {error} />
          {/await}
        {:else if nav.current === 'sessions'}
          {#await lazyPage(() => import('../sessions/SessionsView.svelte')) then { default: SessionsView }}
            <SessionsView />
          {:catch error}
            <ErrorPanel area="The {meta.label} page" {error} />
          {/await}
        {:else if nav.current === 'changes'}
          {#await lazyPage(() => import('../changes/ChangesView.svelte')) then { default: ChangesView }}
            <ChangesView />
          {:catch error}
            <ErrorPanel area="The {meta.label} page" {error} />
          {/await}
        {:else if nav.current === 'settings'}
          <SettingsView />
        {:else if nav.current === 'config'}
          <ConfigViewer />
        {:else if nav.current === 'extensions'}
          <ExtensionsViewer />
        {:else if nav.current === 'ecosystem'}
          <EcosystemView />
        {:else}
          <PlaceholderView view={meta} />
        {/if}
      </ErrorBoundary>
    </div>
  {/key}
</div>

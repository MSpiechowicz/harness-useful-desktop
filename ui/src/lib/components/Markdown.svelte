<script lang="ts">
  import { openExternal } from '$lib/ipc/actions';
  import { toSegments } from '$lib/markdown/render';

  import CodeBlock from './CodeBlock.svelte';

  interface Props {
    source: string;
    /** The text is still arriving; code blocks wait for a pause before highlighting. */
    streaming?: boolean;
  }

  let { source, streaming = false }: Props = $props();

  const segments = $derived(toSegments(source));

  /** Links are buttons: they ask the backend (which enforces the allowlist) to open web links and never navigate. */
  function onclick(event: MouseEvent): void {
    const link = event.target instanceof Element ? event.target.closest('button[data-href]') : null;
    if (!link) {
      return;
    }

    event.preventDefault();
    const href = link.getAttribute('data-href') ?? '';

    if (/^https?:\/\//i.test(href)) {
      void openExternal(href);
    }
  }
</script>

<!-- Clicks are delegated from the rendered links, which are real (keyboard-reachable) buttons. -->
<div class="md" {onclick} role="presentation">
  {#each segments as segment, index (index)}
    {#if segment.type === 'html'}
      <div class="md-html">{@html segment.html}</div><!-- guard-allow: html-injection -->
    {:else}
      <CodeBlock class="bleed" code={segment.code} lang={segment.lang} {streaming} />
    {/if}
  {/each}
</div>

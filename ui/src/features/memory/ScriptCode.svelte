<script lang="ts">
  import CodeBlock from '$lib/components/CodeBlock.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import HiddenText from './HiddenText.svelte';
  import { grammarOf, revealHidden } from './script-format';

  interface Props {
    body: string;
    language: string;
  }

  let { body, language }: Props = $props();

  const hidden = $derived(revealHidden(body));
  // What gets copied is what is shown: the escapes, never the invisible characters themselves.
  const spelledOut = $derived(hidden.segments.map((part) => part.text).join(''));
</script>

<!-- A script with invisible characters is shown plain, with each one spelled out and a warning above it: highlighting
     would hide them, and a script that hides what it does has to be read with care. -->
{#if hidden.count > 0}
  <p class="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">{tn('memory.scripts.body.hidden', hidden.count)}</p>
  <figure class="chat-radius overflow-hidden border border-line bg-canvas">
    <figcaption class="flex h-8 items-center justify-between border-b border-line bg-surface-2 pr-[9px] pl-3.5 text-xs text-fg-2">
      <span class="font-mono font-medium">{grammarOf(language) || 'text'}</span>
      <CopyButton text={spelledOut} label={t('components.copy.code')} />
    </figcaption>
    <pre class="px-3.5 py-3 font-mono text-[13px] leading-5 [overflow-wrap:anywhere] whitespace-pre-wrap text-fg"><code><HiddenText text={body} /></code></pre>
  </figure>
{:else}
  <CodeBlock code={body} lang={grammarOf(language)} />
{/if}

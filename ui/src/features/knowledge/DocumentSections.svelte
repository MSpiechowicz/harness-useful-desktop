<script lang="ts">
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import { scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeSection } from '$lib/ipc/bindings/KnowledgeSection';

  import { outlineOf } from './knowledge-format';

  interface Props {
    sections: KnowledgeSection[];
  }

  let { sections: chunks }: Props = $props();

  const sections = $derived(outlineOf(chunks));

  /** How many sections show before the rest is asked for. */
  const SHOWN = 8;

  /** One level of the outline, in px. */
  const INDENT_PX = 14;

  let all = $state(false);

  const shown = $derived(all ? sections : sections.slice(0, SHOWN));

  /** A heading path as the levels it has and its own title: `Guide > Install > Linux` is the third level, "Linux". */
  function levelOf(section: KnowledgeSection): { depth: number; title: string } {
    const parts = section.headingPath.split('>').map((part) => part.trim()).filter(Boolean);

    return { depth: Math.max(0, parts.length - 1), title: parts.at(-1) ?? t('knowledge.sections.intro') };
  }
</script>

{#if sections.length === 0}
  <p class="text-sm text-fg-3">{t('knowledge.sections.none')}</p>
{:else}
  <ul class="flex flex-col">
    {#each shown as section (section.chunkId)}
      {@const level = levelOf(section)}
      <li transition:slide={{ duration: scaled(160) }} class="flex min-h-8 items-center py-1 text-sm text-fg-2" style:padding-left="{level.depth * INDENT_PX}px" title={section.headingPath}>
        <span class="min-w-0 truncate {level.depth === 0 ? 'text-fg' : ''}">{level.title}</span>
      </li>
    {/each}
  </ul>

  {#if sections.length > SHOWN}
    <div class="-ml-3 pt-1">
      <Button variant="ghost" onclick={() => (all = !all)}>{all ? t('knowledge.sections.fewer') : tn('knowledge.sections.all', sections.length)}</Button>
    </div>
  {/if}
{/if}

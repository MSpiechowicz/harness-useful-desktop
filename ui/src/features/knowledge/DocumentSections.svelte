<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeSection } from '$lib/ipc/bindings/KnowledgeSection';
  import { idKey } from '$lib/ipc/knowledge-id';
  import { uid } from '$lib/utils/ids';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import ChunkParts from './ChunkParts.svelte';
  import { headingLevel, outlineGroups } from './knowledge-format';

  interface Props {
    sections: KnowledgeSection[];
    /** Where the text of a section is read from. */
    texts: ChunkTexts;
  }

  let { sections: chunks, texts }: Props = $props();

  const sections = $derived(outlineGroups(chunks));

  /** How many sections show before the rest is asked for. */
  const SHOWN = 8;

  /** One level of the outline, in px. */
  const INDENT_PX = 14;

  const prefix = uid('section');

  let all = $state(false);
  /** The sections whose text is open, by the first chunk of each. */
  let open = $state.raw<Record<string, boolean>>({});

  const shown = $derived(all ? sections : sections.slice(0, SHOWN));

  function toggle(first: string): void {
    open = { ...open, [first]: !open[first] };
  }
</script>

{#if sections.length === 0}
  <p class="text-sm text-fg-3">{t('knowledge.sections.none')}</p>
{:else}
  <ul class="-mx-2 flex flex-col gap-0.5">
    {#each shown as section (idKey(section.chunkIds[0]))}
      {@const level = headingLevel(section.headingPath)}
      {@const first = idKey(section.chunkIds[0])}
      <!-- An open section is one card: its row and its text share the box, so the hover and the text line up. -->
      <li transition:slide={{ duration: scaled(160) }} class="rounded-md transition-colors duration-[var(--dur-fast)] {open[first] ? 'bg-surface-2' : ''}" style:margin-left="{level.depth * INDENT_PX}px">
        <button
          type="button"
          aria-expanded={open[first] === true}
          aria-controls="{prefix}-{first}"
          title={section.headingPath}
          class="group flex min-h-8 w-full items-center gap-1.5 rounded-md px-2 py-1 text-left text-sm text-fg-2 transition-colors duration-[var(--dur-fast)] hover:text-fg {open[first] ? '' : 'hover:bg-surface-2'}"
          onclick={() => toggle(first)}
        >
          <ChevronRight size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)] {open[first] ? 'rotate-90' : ''}" />
          <span class="min-w-0 truncate {level.depth === 0 ? 'text-fg' : ''}">{level.title}</span>
        </button>

        <div id="{prefix}-{first}">
          {#if open[first]}
            <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
              <div class="pt-1.5 pr-3 pb-3 pl-7.5"><ChunkParts {texts} chunkIds={section.chunkIds} /></div>
            </div>
          {/if}
        </div>
      </li>
    {/each}
  </ul>

  {#if sections.length > SHOWN}
    <div class="-ml-2 pt-1">
      <Button variant="ghost" onclick={() => (all = !all)}>{all ? t('knowledge.sections.fewer') : tn('knowledge.sections.all', sections.length)}</Button>
    </div>
  {/if}
{/if}

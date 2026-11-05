<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { tick } from 'svelte';
  import { slide } from 'svelte/transition';

  import { ICON } from '$lib/components/icon';
  import Pagination from '$lib/components/Pagination.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeSection } from '$lib/ipc/bindings/KnowledgeSection';
  import { idKey } from '$lib/ipc/knowledge-id';
  import { formatCount } from '$lib/utils/format';
  import { uid } from '$lib/utils/ids';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import ChunkParts from './ChunkParts.svelte';
  import { filterOutline, headingLevel, type OutlineGroup, outlineGroups } from './knowledge-format';
  import OutlineRail from './OutlineRail.svelte';

  interface Props {
    sections: KnowledgeSection[];
    /** Where the text of a section is read from. */
    texts: ChunkTexts;
  }

  let { sections: chunks, texts }: Props = $props();

  /** A document with more sections than this gets the filter and the pages. */
  const PAGE_SIZE = 20;

  /** One level of the outline, in px. */
  const INDENT_PX = 14;

  const prefix = uid('section');

  const sections = $derived(outlineGroups(chunks));
  const long = $derived(sections.length > PAGE_SIZE);

  let query = $state('');
  let page = $state(0);
  let list: HTMLElement | undefined = $state();
  /** The sections whose text is open, by the first chunk of each. */
  let open = $state.raw<Record<string, boolean>>({});
  /** The part each section is on, by the first chunk of each. */
  let parts = $state.raw<Record<string, number>>({});

  const found = $derived(long ? filterOutline(sections, query) : sections);
  const filtering = $derived(long && query.trim() !== '');
  const lastPage = $derived(Math.max(0, Math.ceil(found.length / PAGE_SIZE) - 1));
  const onPage = $derived(Math.min(page, lastPage));
  const shown = $derived(long ? found.slice(onPage * PAGE_SIZE, (onPage + 1) * PAGE_SIZE) : found);

  /** The sections whose text has been read, or has waited long enough: the row slides open to the height of what it holds. */
  let revealed = $state.raw<Record<string, boolean>>({});

  async function toggle(section: OutlineGroup): Promise<void> {
    const first = idKey(section.chunkIds[0]);
    const opening = !open[first];

    open = { ...open, [first]: opening };

    if (opening) {
      await texts.settle(section.chunkIds[Math.min(parts[first] ?? 0, section.chunkIds.length - 1)]);
    }

    if (open[first] === opening) {
      revealed = { ...revealed, [first]: opening };
    }
  }

  async function turn(next: number): Promise<void> {
    page = next;
    await tick();
    list?.scrollIntoView?.({ block: 'nearest' });
  }
</script>

{#if sections.length === 0}
  <p class="text-sm text-fg-3">{t('knowledge.sections.none')}</p>
{:else}
  {#if long}
    <SearchField
      class="mb-2"
      bind:value={() => query, (value) => {
        query = value;
        page = 0;
      }}
      label={t('knowledge.sections.filter')}
      placeholder={t('knowledge.sections.filter')}
      status={t('knowledge.sections.found', { count: formatCount(found.length), total: formatCount(sections.length) })}
    />
  {/if}

  {#if found.length === 0}
    <p class="py-2 text-sm text-fg-3">{t('knowledge.sections.noMatch')}</p>
  {:else}
    <ul bind:this={list} class="-mx-2 flex scroll-mt-16 flex-col gap-0.5">
      {#each shown as section (idKey(section.chunkIds[0]))}
        {@const level = headingLevel(section.headingPath)}
        {@const first = idKey(section.chunkIds[0])}
        <!-- A filtered list has lost its outline, so its rows do not step in. -->
        <li style:margin-left="{filtering ? 0 : level.depth * INDENT_PX}px">
          <button
            type="button"
            aria-expanded={open[first] === true}
            aria-controls="{prefix}-{first}"
            title={section.headingPath}
            class="group flex min-h-control w-full items-center gap-1.5 rounded-md px-2 py-1 text-left text-sm text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-surface-2 hover:text-fg"
            onclick={() => void toggle(section)}
          >
            <ChevronRight size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)] {open[first] ? 'rotate-90' : ''}" />
            <span class="min-w-0 truncate {level.depth === 0 || open[first] ? 'text-fg' : ''}">{level.title}</span>
          </button>

          <div id="{prefix}-{first}">
            {#if open[first] && revealed[first]}
              <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
                <OutlineRail>
                  <ChunkParts
                    {texts}
                    chunkIds={section.chunkIds}
                    bind:index={() => parts[first] ?? 0, (part) => (parts = { ...parts, [first]: part })}
                  />
                </OutlineRail>
              </div>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if long && found.length > 0}
    <Pagination compact page={onPage} pageSize={PAGE_SIZE} total={found.length} onpage={(next) => void turn(next)} />
  {/if}
{/if}

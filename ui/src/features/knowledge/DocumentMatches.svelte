<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeHit } from '$lib/ipc/bindings/KnowledgeHit';
  import { ipc } from '$lib/ipc/client';
  import { IpcFailure } from '$lib/ipc/transport';
  import { uid } from '$lib/utils/ids';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import ChunkPart from './ChunkPart.svelte';
  import { headingLevel } from './knowledge-format';
  import { highlightPieces } from './knowledge-text';

  interface Props {
    docId: number;
    /** The search of the documents table, once typing settled. */
    query: string;
    texts: ChunkTexts;
  }

  let { docId, query, texts }: Props = $props();

  /** How many passages the search is asked for across all documents: this document shows the ones that are its own. */
  const LIMIT = 20;

  const prefix = uid('match');

  /** The passages of this document, or null before the first answer. A newer search keeps showing the last ones until its answer is in. */
  let hits = $state.raw<KnowledgeHit[] | null>(null);
  let error = $state<string | null>(null);
  let attempt = $state(0);
  let open = $state.raw<Record<number, boolean>>({});

  // Another document starts with no passages of the last one.
  $effect(() => {
    void docId;

    hits = null;
    open = {};
  });

  // Every search asks again, and an answer to an older one is dropped.
  $effect(() => {
    const asked = query.trim();
    const id = docId;
    let stale = false;

    void attempt;
    error = null;

    ipc
      .invoke('knowledge_search', { query: asked, project: null, scope: 'all', limit: LIMIT })
      .then((found) => {
        if (!stale) {
          hits = found.filter((hit) => hit.docId === id);
        }
      })
      .catch((failure: unknown) => {
        if (!stale) {
          error = IpcFailure.from(failure).message;
        }
      });

    return () => {
      stale = true;
    };
  });

  function toggle(chunkId: number): void {
    open = { ...open, [chunkId]: !open[chunkId] };
  }
</script>

{#if error !== null}
  <div class="flex flex-col items-start gap-3">
    <p class="text-sm text-danger" role="alert">{error}</p>
    <Button onclick={() => (attempt += 1)}>{t('memory.tryAgain')}</Button>
  </div>
{:else if hits === null}
  <div class="flex flex-col gap-2" role="status" aria-label={t('knowledge.matches.loading')}>
    <Skeleton height="1.25rem" width="40%" />
    <Skeleton height="1.25rem" />
  </div>
{:else if hits.length === 0}
  <p class="text-sm text-fg-3">{t('knowledge.matches.none')}</p>
{:else}
  <ul class="-mx-2 flex flex-col gap-0.5">
    {#each hits as hit (hit.chunkId)}
      <li class="rounded-md transition-colors duration-[var(--dur-fast)] {open[hit.chunkId] ? 'bg-surface-2' : ''}">
        <button
          type="button"
          aria-expanded={open[hit.chunkId] === true}
          aria-controls="{prefix}-{hit.chunkId}"
          title={hit.section}
          class="flex w-full items-start gap-1.5 rounded-md px-2 py-1.5 text-left text-sm transition-colors duration-[var(--dur-fast)] {open[hit.chunkId] ? '' : 'hover:bg-surface-2'}"
          onclick={() => toggle(hit.chunkId)}
        >
          <ChevronRight size={ICON.sm} aria-hidden="true" class="mt-0.5 shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)] {open[hit.chunkId] ? 'rotate-90' : ''}" />
          <span class="flex min-w-0 flex-col gap-0.5">
            <span class="truncate text-fg">{headingLevel(hit.section).title}</span>
            <span class="line-clamp-3 text-fg-2">
              {#each highlightPieces(hit.snippet, query) as piece, index (index)}{#if piece.match}<mark class="rounded-sm bg-accent-wash text-fg">{piece.text}</mark>{:else}{piece.text}{/if}{/each}
            </span>
          </span>
        </button>

        <div id="{prefix}-{hit.chunkId}">
          {#if open[hit.chunkId]}
            <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
              <div class="pr-3 pb-3 pl-7.5"><ChunkPart {texts} chunkId={hit.chunkId} {query} inset /></div>
            </div>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
{/if}

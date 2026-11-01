<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import SearchX from '@lucide/svelte/icons/search-x';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeDocMatch } from '$lib/ipc/bindings/KnowledgeDocMatch';
  import { ipc } from '$lib/ipc/client';
  import { idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
  import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
  import { uid } from '$lib/utils/ids';
  import { readFailureText } from '$lib/utils/knowledge-failure';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import ChunkPart from './ChunkPart.svelte';
  import { headingLevel } from './knowledge-format';
  import { highlightPieces } from './knowledge-text';

  interface Props {
    docId: KnowledgeId;
    /** The search of the documents table, once typing settled. */
    query: string;
    texts: ChunkTexts;
  }

  let { docId, query, texts }: Props = $props();

  /** How many passages one document lists. A server without `doc-matches` is asked for this many across all documents instead and this document shows the ones that are its own. */
  const LIMIT = 20;

  const prefix = uid('match');

  /** The passages of this document, or null before the first answer. A newer search keeps showing the last ones until its answer is in. */
  let hits = $state.raw<KnowledgeDocMatch[] | null>(null);
  /** How many passages match in all, which can be more than the ones listed. */
  let total = $state(0);
  let error = $state<string | null>(null);
  let attempt = $state(0);
  let open = $state.raw<Record<string, boolean>>({});

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
    void knowledgeStatus.value;
    error = null;

    /** What the server can do is read once, and the first search waits for it. A server that cannot say is asked the old way. */
    async function ask(): Promise<{ matches: KnowledgeDocMatch[]; total: number }> {
      if (knowledgeStatus.value === null) {
        await knowledgeStatus.load();

        // The status arriving ran this search again: that run asks.
        if (stale) {
          return { matches: [], total: 0 };
        }
      }

      if (knowledgeStatus.value?.features.docMatches) {
        return ipc.invoke('knowledge_doc_matches', { id, query: asked, limit: LIMIT });
      }

      const all = await ipc.invoke('knowledge_search', { query: asked, project: null, scope: 'all', limit: LIMIT });
      const own = all.filter((hit) => idKey(hit.docId) === idKey(id)).map((hit) => ({ chunkId: hit.chunkId, ordinal: 0, section: hit.section, snippet: hit.snippet }));

      return { matches: own, total: own.length };
    }

    ask()
      .then((answer) => {
        if (!stale) {
          hits = answer.matches;
          total = answer.total;
        }
      })
      .catch((failure: unknown) => {
        if (!stale) {
          error = readFailureText(failure);
        }
      });

    return () => {
      stale = true;
    };
  });

  function toggle(chunkId: KnowledgeId): void {
    const key = idKey(chunkId);

    open = { ...open, [key]: !open[key] };
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
  <!-- A result, not a hint: it reads as a state under the section's description. -->
  <p class="flex items-start gap-2 rounded-md bg-surface-2 px-3 py-2 text-sm text-fg-2" role="status">
    <SearchX size={ICON.sm} class="mt-0.5 shrink-0 text-fg-3" aria-hidden="true" />
    {t('knowledge.matches.none')}
  </p>
{:else}
  <p class="pb-1.5 text-xs text-fg-3" role="status">{tn('knowledge.matches.count', total)}</p>
  <ul class="-mx-2 flex flex-col gap-0.5">
    {#each hits as hit (idKey(hit.chunkId))}
      {@const key = idKey(hit.chunkId)}
      <li class="rounded-md transition-colors duration-[var(--dur-fast)] {open[key] ? 'bg-surface-2' : ''}">
        <button
          type="button"
          aria-expanded={open[key] === true}
          aria-controls="{prefix}-{key}"
          title={hit.section}
          class="flex w-full items-start gap-1.5 rounded-md px-2 py-1.5 text-left text-sm transition-colors duration-[var(--dur-fast)] {open[key] ? '' : 'hover:bg-surface-2'}"
          onclick={() => toggle(hit.chunkId)}
        >
          <ChevronRight size={ICON.sm} aria-hidden="true" class="mt-0.5 shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)] {open[key] ? 'rotate-90' : ''}" />
          <span class="flex min-w-0 flex-col gap-0.5">
            <span class="truncate text-fg">{headingLevel(hit.section).title}</span>
            <span class="line-clamp-3 text-fg-2">
              {#each highlightPieces(hit.snippet, query) as piece, index (index)}{#if piece.match}<mark class="rounded-sm bg-accent-wash text-fg">{piece.text}</mark>{:else}{piece.text}{/if}{/each}
            </span>
          </span>
        </button>

        <div id="{prefix}-{key}">
          {#if open[key]}
            <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
              <div class="pt-1.5 pr-3 pb-3 pl-7.5"><ChunkPart {texts} chunkId={hit.chunkId} {query} inset /></div>
            </div>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
{/if}

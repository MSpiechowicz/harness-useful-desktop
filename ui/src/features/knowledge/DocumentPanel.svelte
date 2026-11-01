<script lang="ts">
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import Button from '$lib/components/Button.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import type { ScopeOption } from '$lib/components/scope/scope-options';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeDetail } from '$lib/ipc/bindings/KnowledgeDetail';
  import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
  import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';

  import BodyState from '../memory/BodyState.svelte';
  import { settled } from '../memory/memory-data.svelte';
  import { ChunkTexts } from './chunk-texts.svelte';
  import DocumentMatches from './DocumentMatches.svelte';
  import DocumentSections from './DocumentSections.svelte';
  import DocumentTitle from './DocumentTitle.svelte';
  import { docFacts, docSubtitle, outlineOf } from './knowledge-format';
  import LinksEditor from './LinksEditor.svelte';

  interface Props {
    open: boolean;
    /** The panel is a column of a wide view (not a slide-over). */
    inline: boolean;
    /** The document as the list or its own read gave it. Null while one that is not on this page is being read. */
    doc: KnowledgeDocument | null;
    /** The document with its outline; null until it was read. */
    detail: KnowledgeDetail | null;
    options: ScopeOption[];
    /** How the read of the outline stands, and a way to read it again. */
    status: ViewStatus;
    error: string | null;
    onretry: () => void;
    onrename: (title: string) => Promise<boolean>;
    /** Saves the links and answers them as stored; null when the server refused. */
    onlinks: (links: KnowledgeLink[]) => Promise<KnowledgeLink[] | null>;
    ondelete: () => void;
    onclose: () => void;
    /** The search of the documents table as typed: while it holds words, the panel shows where they are in the document. */
    query: string;
  }

  let { open, inline, doc, detail, status, error, options, query, onretry, onrename, onlinks, ondelete, onclose }: Props = $props();

  const searched = settled(() => query);
  const searching = $derived(searched.value.trim() !== '');

  /** The texts of this document's chunks, read as the user opens them. */
  const docId = $derived(doc?.id ?? null);
  const texts = $derived(docId === null ? null : new ChunkTexts(docId));

  let renaming = $state(false);
  /** The links as the user left them while the server has not answered yet, and whether the last change was saved. */
  let draft = $state.raw<KnowledgeLink[] | null>(null);
  let saved = $state(false);
  let sent = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Another document starts from what it has, with no field open.
  $effect(() => {
    void doc?.id;

    renaming = false;
    draft = null;
    saved = false;
  });

  async function change(next: KnowledgeLink[]): Promise<void> {
    sent += 1;
    const mine = sent;

    draft = next;
    saved = false;
    clearTimeout(timer);

    const stored = await onlinks(next);

    // A newer change owns what is on screen.
    if (mine !== sent) {
      return;
    }

    draft = null;
    saved = stored !== null;
    timer = setTimeout(() => (saved = false), 2000);
  }
</script>

{#snippet rename()}
  <IconButton icon={Pencil} iconSize={ICON.sm} label={t('knowledge.panel.rename')} onclick={() => (renaming = true)} />
{/snippet}

{#snippet field()}
  {#if doc}
    <DocumentTitle title={doc.title} onsave={onrename} onleave={() => (renaming = false)} />
  {/if}
{/snippet}

<SidePanel {open} {inline} title={doc?.title ?? ''} subtitle={doc ? docSubtitle(doc) : ''} actions={doc ? rename : undefined} titleField={renaming ? field : undefined} {onclose}>
  {#if doc}
    {#if doc.warnings.length > 0}
      <ul class="flex flex-col gap-1 rounded-md bg-warning-wash px-3 py-2 text-sm text-warning" role="status">
        {#each doc.warnings as warning (warning)}
          <li>{warning}</li>
        {/each}
      </ul>
    {/if}

    {#if searching && texts}
      <SectionCard flat heading={t('knowledge.panel.matches')} hint={t('knowledge.matches.hint')}>
        <DocumentMatches docId={doc.id} query={searched.value} {texts} />
      </SectionCard>
    {/if}

    <SectionCard flat heading={t('knowledge.panel.links')}>
      <LinksEditor links={draft ?? doc.links} onchange={(next) => void change(next)} {options} {saved} globalNote={t('knowledge.links.global')} />
    </SectionCard>

    <SectionCard flat heading={t('knowledge.panel.sections')} hint={t('knowledge.sections.hint')}>
      <BodyState loaded={detail !== null} {status} {error} failed={t('knowledge.panel.sections.failed')} loading={t('knowledge.panel.sections.loading')} {onretry}>
        {#if texts}<DocumentSections sections={detail?.sections ?? []} {texts} />{/if}
      </BodyState>
    </SectionCard>

    <FactsCard flat heading={t('knowledge.panel.details')} rows={docFacts(doc, detail ? outlineOf(detail.sections).length : null)} />

    <SectionCard flat heading={t('knowledge.panel.actions')}>
      <Button variant="dangerQuiet" onclick={ondelete}>
        {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('knowledge.panel.delete')}
      </Button>
    </SectionCard>
  {/if}
</SidePanel>

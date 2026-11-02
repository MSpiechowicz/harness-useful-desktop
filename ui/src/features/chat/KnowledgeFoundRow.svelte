<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { docLookup } from '$lib/chat/doc-lookup.svelte';
  import type { FoundDoc } from '$lib/chat/knowledge-tools';
  import type { RowMarks } from '$lib/chat/use-index';
  import { t } from '$lib/i18n/i18n.svelte';

  import { formatLabel, formatTone, linksText } from '../knowledge/knowledge-format';

  import FamilyFoundRow from './FamilyFoundRow.svelte';
  import { showDocument } from './knowledge-open';

  interface Props extends RowMarks {
    /** A document a reply named. A row with no title is one only its id is known of: the title comes from Knowledge once it is read, else it is "Document <id>". */
    row: FoundDoc;
  }

  let { row, use, dim = false }: Props = $props();

  // A search names a title and a section but not the format, the place or the repositories: they are read from Knowledge and shown once known.
  const doc = $derived(docLookup.get(row.id));

  $effect(() => {
    void docLookup.ensure(row.id);
  });

  const format = $derived(doc?.format ?? row.format);
  const title = $derived(row.title || doc?.title || t('chat.knowledge.row.unknown', { id: row.id }));

  /** Where it is kept (only when both places are in use) and the repositories it applies to, or Global. Nothing while the document is unknown. */
  const meta = $derived.by(() => {
    if (!doc) {
      return '';
    }

    const place = doc.place ? [t(doc.place === 'local' ? 'chat.knowledge.row.place.local' : 'chat.knowledge.row.place.remote')] : [];

    return [...place, linksText(doc.links)].join(' · ');
  });
</script>

{#snippet badge()}
  {#if format}<Badge tone={formatTone(format)} size="lg">{formatLabel(format)}</Badge>{/if}
{/snippet}

<FamilyFoundRow {badge} {title} {meta} {use} {dim} showLabel={t('chat.knowledge.row.show')} onshow={() => showDocument(row.id)} />

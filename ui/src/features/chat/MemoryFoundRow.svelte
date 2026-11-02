<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import type { FoundRow, UnknownNote } from '$lib/chat/memory-tools';
  import type { RowMarks } from '$lib/chat/use-index';
  import { toolContext } from '$lib/chat/tool-context';
  import { t } from '$lib/i18n/i18n.svelte';
  import { perLocale } from '$lib/utils/format';

  import NoteKindBadge from '../memory/NoteKindBadge.svelte';
  import { showNote, showScript } from '../memory/memory-open';
  import { languageLabel } from '../memory/script-format';

  import FamilyFoundRow from './FamilyFoundRow.svelte';

  interface Props extends RowMarks {
    /** A note or script a search listed, or a note only its id is known of (no badge and no details, just `#<id>`). */
    row: FoundRow | UnknownNote;
  }

  let { row, use, dim = false }: Props = $props();

  const where = toolContext();

  const DAY = perLocale((l) => new Intl.DateTimeFormat(l, { day: 'numeric', month: 'short', timeZone: 'UTC' }));
  const DAY_YEAR = perLocale((l) => new Intl.DateTimeFormat(l, { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' }));

  /** "14 Oct", with the year once it is not this one. */
  function shortDate(day: string): string {
    const date = new Date(`${day}T00:00:00Z`);
    if (Number.isNaN(date.getTime())) {
      return day;
    }

    return (date.getUTCFullYear() === new Date().getUTCFullYear() ? DAY() : DAY_YEAR()).format(date);
  }

  /** A repository note says the folder it was written from, a global one says so; "Repository" only when the server named no folder. */
  function whereOf(note: { scope: 'repo' | 'global'; path: string | null }): string {
    if (note.scope === 'global') {
      return t('chat.memory.row.scope.global');
    }

    return note.path ?? t('chat.memory.row.scope.repo');
  }

  const meta = $derived.by(() => {
    if (row.type === 'unknown') {
      return '';
    }

    if (row.type === 'script') {
      return row.description;
    }

    return `${whereOf(row)} · ${shortDate(row.date)}`;
  });
  const title = $derived.by(() => {
    if (row.type === 'unknown') {
      return `#${row.id}`;
    }

    return row.type === 'note' ? row.title : row.name;
  });

  function show(): void {
    if (row.type === 'unknown') {
      showNote(row.id);
    } else if (row.type === 'note') {
      showNote(row.id, row.scope);
    } else {
      void showScript(row.id ?? row.name, where?.project ?? null);
    }
  }
</script>

{#snippet badge()}
  {#if row.type === 'note'}
    <NoteKindBadge kind={row.kind} size="lg" />
  {:else if row.type === 'script'}
    <Badge size="lg">{languageLabel(row.language)}</Badge>
  {/if}
{/snippet}

<FamilyFoundRow {badge} {title} {meta} {use} {dim} showLabel={t('chat.memory.row.show')} onshow={show} />

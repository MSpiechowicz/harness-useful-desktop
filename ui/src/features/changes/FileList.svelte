<script lang="ts">
  import FileX from '@lucide/svelte/icons/file-x';

  import ChangeList from '$lib/components/ChangeList.svelte';
  import type { ChangeItem, ChangeSection } from '$lib/components/change-list';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import { entrySections, SCOPE_LABELS, SECTIONS, statusLetter, statusName, statusTone, type Section } from '$lib/git/entries';
  import { normalizeQuery } from '$lib/git/search';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
  import { git } from '$lib/stores/git.svelte';
  import { splitPath } from '$lib/utils/paths';

  import { folded } from './sections.svelte';

  interface Props {
    entries: StatusEntry[];
    /** Pending review comments per path. */
    commentCounts: ReadonlyMap<string, number>;
    /** Called after a click chooses a file, e.g. to put a drawer away. */
    onpick?: () => void;
    /** Asks to discard a file's changes (the confirmation lives with the view). */
    ondiscard: (entry: StatusEntry) => void;
  }

  let { entries, commentCounts, onpick, ondiscard }: Props = $props();

  interface Row {
    entry: StatusEntry;
    section: Section;
  }

  const keyOf = (row: Row): string => `${row.section}:${row.entry.path}`;

  /** The files under each heading; a file that is both staged and edited is listed under both. */
  const groups = $derived(
    SECTIONS.map((section) => ({ section, rows: entries.filter((entry) => entrySections(entry).includes(section)).map((entry): Row => ({ entry, section })) })).filter((group) => group.rows.length > 0),
  );

  const rowsByKey = $derived(new Map(groups.flatMap((group) => group.rows).map((row) => [keyOf(row), row])));

  /** While searching, every heading stays open so no match is hidden behind a fold. */
  const searching = $derived(normalizeQuery(git.search) !== null);

  /** Added and removed lines of a file under a heading, once its diff has loaded. */
  function stats(path: string, section: Section): { additions: number; deletions: number } | null {
    const state = git.diffFor(path);
    if (state?.status !== 'ready') {
      return null;
    }

    const own = state.diffs.filter((diff) => diff.staged === (section === 'staged'));
    const shown = own.length > 0 ? own : state.diffs;
    if (shown.length === 0 || shown.every((diff) => diff.isBinary)) {
      return null;
    }

    return {
      additions: shown.reduce((sum, diff) => sum + diff.additions, 0),
      deletions: shown.reduce((sum, diff) => sum + diff.deletions, 0),
    };
  }

  function itemOf(row: Row): ChangeItem {
    const { entry, section } = row;
    const status = (section === 'staged' ? entry.staged : entry.unstaged) ?? entry.staged ?? 'modified';
    const counts = stats(entry.path, section);

    return {
      id: keyOf(row),
      label: entry.path,
      sublabel: entry.origPath ? `${entry.origPath} → ${entry.path}` : undefined,
      letter: statusLetter(status),
      letterLabel: statusName(status),
      tone: statusTone(status),
      additions: counts?.additions,
      deletions: counts?.deletions,
      comments: commentCounts.get(entry.path) ?? 0,
      hits: git.hits.get(entry.path)?.lines,
    };
  }

  const sections = $derived<ChangeSection[]>(groups.map((group) => ({ id: group.section, title: SCOPE_LABELS[group.section], items: group.rows.map(itemOf) })));

  function select(id: string): void {
    const row = rowsByKey.get(id);
    if (row) {
      git.select(row.entry.path, row.section);
    }
  }
</script>

<ChangeList
  {sections}
  selectedId={git.selected === null ? null : `${git.section}:${git.selected}`}
  label={t('changes.files.label')}
  testid="file-list"
  locked={searching}
  isOpen={(id) => searching || !folded.has(id as Section)}
  ontoggle={(id) => folded.toggle(id as Section)}
  onselect={select}
  onpick={() => onpick?.()}
>
  {#snippet menu(item, state)}
    {@const row = rowsByKey.get(item.id)}
    <MoreMenu label={t('changes.files.menu', { name: splitPath(item.label).name })} bind:open={() => state.open, state.setOpen} panelClass="w-56">
      {#snippet children({ close })}
        <MenuItem
          tone="danger"
          icon={FileX}
          onclick={() => {
            close();
            if (row) {
              ondiscard(row.entry);
            }
          }}>{t('changes.discardFile.action')}</MenuItem
        >
      {/snippet}
    </MoreMenu>
  {/snippet}
</ChangeList>

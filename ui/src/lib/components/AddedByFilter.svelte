<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { AuthorFacet } from '$lib/ipc/bindings/AuthorFacet';
  import type { AuthorFilter } from '$lib/ipc/bindings/AuthorFilter';
  import { AUTHOR_SEARCH_FROM, authorFromValue, authorMenuShown, authorOptions, authorValue } from '$lib/utils/author-filter';

  import FilterMenu from './FilterMenu.svelte';

  interface Props {
    /** Who added the entries under the list's other filters, with a count each; null from a server that does not send it. */
    facet: AuthorFacet | null;
    /** The author the list is narrowed to, null for everyone. */
    author: AuthorFilter | null;
    /** Whether the server filters by author at all. */
    supported: boolean;
    /** Whether the list is narrowed some way (a search, any filter, this one too): the menu then stays even when no one is left to choose. */
    narrowed: boolean;
    /** Called with the author chosen, or null for everyone, so the view can go back to its first page. */
    onchange: (author: AuthorFilter | null) => void;
  }

  let { facet, author, supported, narrowed, onchange }: Props = $props();

  const options = $derived(authorOptions(facet, author, { all: t('memory.filter.all'), unknown: t('components.addedBy.unknown') }));
</script>

<!-- The same "Added by" filter wherever a table lists what people or their agents added: notes, scripts and documents. The people are many in a big team, so a long menu is searchable. -->
{#if authorMenuShown(supported, facet, narrowed)}
  <FilterMenu
    label={t('components.addedBy.label')}
    prefix={t('components.addedBy.label')}
    {options}
    value={authorValue(author)}
    onchange={(next) => onchange(authorFromValue(next))}
    onclear={() => onchange(null)}
    clearLabel={t('components.addedBy.clear')}
    align="start"
    search={options.length >= AUTHOR_SEARCH_FROM
      ? { label: t('components.addedBy.search'), placeholder: t('components.addedBy.search.placeholder'), empty: t('components.addedBy.search.empty') }
      : undefined}
  />
{/if}

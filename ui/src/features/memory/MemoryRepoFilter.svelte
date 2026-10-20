<script lang="ts">
  import Folder from '@lucide/svelte/icons/folder';

  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ALL_REPOS, GLOBAL_ONLY, type Reach, reachKey, repoReach } from '$lib/stores/memory-reach';
  import { noteRepos, scriptRepos } from '$lib/stores/memoryRepos.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  interface Props {
    section: 'notes' | 'scripts';
    /** Called after another choice is made, so the view can go back to its first page. */
    onchange: () => void;
  }

  let { section, onchange }: Props = $props();

  const page = $derived((section === 'notes' ? noteRepos : scriptRepos).data);
  const repos = $derived(page?.availability.kind === 'available' ? page.repos : []);
  const global = $derived(page?.availability.kind === 'available' ? page.global : undefined);

  /** `owner/repo` of a repository shown as `host/owner/repo`; a folder (`path:` key) as it is. */
  function shortName(label: string): string {
    const parts = label.split('/');

    return parts.length > 2 && !label.startsWith('/') ? parts.slice(1).join('/') : label;
  }

  const options = $derived<{ value: Reach; label: string; count?: number }[]>([
    { value: ALL_REPOS, label: t('memory.filter.repo.all'), count: page && page.availability.kind === 'available' ? repos.reduce((sum, repo) => sum + repo.count, 0) + (global ?? 0) : undefined },
    ...repos.map((repo) => ({ value: repoReach(repo.key), label: shortName(repo.label), count: repo.count })),
    { value: GLOBAL_ONLY, label: t('memory.filter.repo.global'), count: global },
  ]);

  // The choice is shared by the Notes and the Scripts views: a repository that has none of this kind (or no longer any) is not offered, so it is let go of instead of leaving a filter nothing shows.
  $effect(() => {
    const key = reachKey(viewState.memoryReach);

    if (key !== null && page?.availability.kind === 'available' && !repos.some((repo) => repo.key === key)) {
      viewState.memoryReach = ALL_REPOS;
      onchange();
    }
  });

  function choose(next: Reach): void {
    viewState.memoryReach = next;
    onchange();
  }
</script>

<FilterMenu
  label={t('memory.filter.repo')}
  prefix={t('memory.filter.repo')}
  icon={Folder}
  {options}
  value={viewState.memoryReach}
  onchange={choose}
  onclear={() => choose(ALL_REPOS)}
  clearLabel={t('memory.filter.clearRepo')}
  align="start"
  search={{ label: t('memory.filter.repo.search'), placeholder: t('memory.filter.repo.search.placeholder'), empty: t('memory.filter.repo.search.empty') }}
/>

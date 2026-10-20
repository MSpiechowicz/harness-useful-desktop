<script lang="ts">
  import ScopeFilter from '$lib/components/scope/ScopeFilter.svelte';
  import { buildScopeOptions, type ScopeChoice } from '$lib/components/scope/scope-options';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ALL_REPOS, reachKey } from '$lib/stores/memory-reach';
  import { noteRepos, scriptRepos } from '$lib/stores/memoryRepos.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  interface Props {
    section: 'notes' | 'scripts';
    /** Called after another choice is made, so the view can go back to its first page. */
    onchange: () => void;
  }

  let { section, onchange }: Props = $props();

  const page = $derived((section === 'notes' ? noteRepos : scriptRepos).data);
  const available = $derived(page?.availability.kind === 'available' ? page : null);
  const projects = $derived(projectScopes.data ?? []);

  const options = $derived(
    buildScopeOptions({
      projects,
      remote: available ? available.repos : null,
      globalCount: available?.global,
      allCount: available ? available.repos.reduce((sum, repo) => sum + repo.count, 0) + available.global : undefined,
      labels: { all: t('components.scope.all'), global: t('components.scope.global'), remoteOnly: t('components.scope.remoteOnly') },
    }),
  );

  // The choice is shared by the Notes and the Scripts views: a repository only the source knew, and that has none of this kind any more, is let go of instead of leaving a filter nothing shows.
  // A project on this computer stays chosen with none, so the user can look at an empty repository.
  $effect(() => {
    const choice = viewState.memoryReach;

    if (reachKey(choice) !== null && available && projectScopes.data !== null && !options.some((option) => option.value === choice)) {
      viewState.memoryReach = ALL_REPOS;
      onchange();
    }
  });

  function choose(next: ScopeChoice): void {
    viewState.memoryReach = next;
    onchange();
  }
</script>

<!-- A project without a repository key (Harness Memory could not name it) is listed but cannot be chosen: the notes and scripts are read by key. -->
<ScopeFilter {options} value={viewState.memoryReach as ScopeChoice} onchange={choose} disableKeyless />

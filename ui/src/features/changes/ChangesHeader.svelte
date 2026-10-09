<script lang="ts">
  import Columns2 from '@lucide/svelte/icons/columns-2';
  import Rows2 from '@lucide/svelte/icons/rows-2';
  import X from '@lucide/svelte/icons/x';

  import CostChip from '$lib/components/CostChip.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { SCOPE_LABELS, SCOPES, scopeCounts } from '$lib/git/entries';
  import type { DiffScope } from '$lib/ipc/bindings/DiffScope';
  import type { GitStatus } from '$lib/ipc/bindings/GitStatus';
  import { git } from '$lib/stores/git.svelte';
  import { project } from '$lib/stores/project.svelte';

  import BranchSummary from './BranchSummary.svelte';
  import { diffMode, type DiffMode } from './diff-mode.svelte';

  interface Props {
    status: GitStatus;
  }

  let { status }: Props = $props();

  const costArgs = $derived(status.branch && project.active ? { branch: status.branch, project: project.active.path } : null);
  const counts = $derived(scopeCounts(status.entries));
  const scopes = $derived<StripOption<DiffScope>[]>(SCOPES.map((scope) => ({ value: scope, label: SCOPE_LABELS[scope], count: counts[scope] })));

  const MODES: StripOption<DiffMode>[] = [
    { value: 'unified', label: 'Unified', icon: Rows2 },
    { value: 'split', label: 'Split', icon: Columns2 },
  ];
</script>

<div class="page-gutter flex flex-wrap items-center gap-x-4 gap-y-2 py-2.5" role="toolbar" aria-label="Changes filters">
  <BranchSummary {status} />
  <CostChip kind="branch" args={costArgs} align="start" />

  {#if git.filter}
    <span class="inline-flex items-center gap-1 rounded-full bg-accent-wash py-0.5 pr-1 pl-3 text-xs font-medium text-accent-text" data-testid="touched-filter">
      Touched by {git.filter.label} · {git.filter.paths.length} {git.filter.paths.length === 1 ? 'file' : 'files'}
      <button type="button" aria-label="Show all changes" class="grid size-5 place-items-center rounded-full hover:bg-hover" onclick={() => git.clearFilter()}>
        <X size={16} aria-hidden="true" />
      </button>
    </span>
  {/if}

  <div class="ml-auto flex flex-wrap items-center gap-3">
    <SegmentedControl options={scopes} value={git.scope} label="Which changes" onchange={(scope) => git.setScope(scope)} />
    <SegmentedControl options={MODES} value={diffMode.value} label="Diff layout" onchange={(mode) => diffMode.set(mode)} />
  </div>
</div>

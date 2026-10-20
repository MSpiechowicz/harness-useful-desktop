<script lang="ts">
  import Folder from '@lucide/svelte/icons/folder';
  import Globe from '@lucide/svelte/icons/globe';

  import { t } from '$lib/i18n/i18n.svelte';

  import Badge from '../Badge.svelte';
  import { ICON } from '../icon';

  interface Props {
    /** Where the document applies: everywhere, one repository (named), or unknown, for a file that could not be read. */
    scope: { kind: 'global' } | { kind: 'repo'; label: string } | { kind: 'unknown' };
    /** The document comes from the team, not from a file on this computer. */
    remote?: boolean;
  }

  let { scope, remote = false }: Props = $props();

  const label = $derived(scope.kind === 'repo' ? scope.label : t(scope.kind === 'global' ? 'components.scope.global' : 'components.scope.unknown'));
</script>

<!-- The Scope column of a documents table: a globe for the global ones, a folder and the name for a repository's. -->
<td role="gridcell" class="border-b border-line/60 px-3 text-fg-2">
  <div class="flex min-w-0 items-center gap-1.5">
    {#if scope.kind === 'global'}
      <Globe size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3" />
    {:else if scope.kind === 'repo'}
      <Folder size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3" />
    {/if}
    <span class={scope.kind === 'unknown' ? 'truncate text-fg-3' : 'truncate'} title={label}>{label}</span>
    {#if remote}<Badge class="shrink-0">{t('components.scope.remote')}</Badge>{/if}
  </div>
</td>

<script lang="ts">
  import type { PackageInfo } from '$lib/ipc/bindings/PackageInfo';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import ActionBar from './actions/ActionBar.svelte';
  import { packageActions } from './actions/item-actions';

  interface Props {
    pkg: PackageInfo;
  }

  let { pkg }: Props = $props();

  // Source and contents are facts in the cards above, as on a plugin, so only the actions are left here.
  const actions = $derived(harness.active?.capabilities.cliActions ? packageActions(harness.active.info.id, pkg, project.active?.path ?? null) : []);
</script>

{#if actions.length > 0}
  <ActionBar {actions} />
{/if}

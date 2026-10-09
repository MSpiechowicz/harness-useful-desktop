<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { git } from '$lib/stores/git.svelte';

  interface Props {
    project: string;
    paths: string[];
  }

  let { project, paths }: Props = $props();

  const SHOWN = 6;
  const LABEL = 'this session';
</script>

{#if paths.length > 0}
  <div class="flex flex-wrap items-center gap-2" role="group" aria-label="Files touched">
    <span class="text-xs font-medium text-fg-3">Files touched</span>

    {#each paths.slice(0, SHOWN) as path (path)}
      <button
        type="button"
        class="rounded-sm bg-surface-3 px-2 py-0.5 font-mono text-xs text-fg-2 transition-colors hover:bg-hover hover:text-fg"
        title="Show the changes to {path}"
        onclick={() => git.showPaths(project, [path], LABEL)}
      >
        {path}
      </button>
    {/each}

    {#if paths.length > SHOWN}<span class="text-xs text-fg-3">+{paths.length - SHOWN} more</span>{/if}

    <Button size="sm" variant="ghost" onclick={() => git.showPaths(project, paths, LABEL)}>View changes</Button>
  </div>
{/if}

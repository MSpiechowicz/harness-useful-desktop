<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  import Button from '$lib/components/Button.svelte';
  import ProjectRow from '$lib/components/ProjectRow.svelte';
  import { project } from '$lib/stores/project.svelte';

  import StepHeader from './StepHeader.svelte';
</script>

<div class="flex flex-col gap-6">
  <StepHeader
    eyebrow="Your project"
    title="Pick a project to work in"
    body="Config and extensions can include project-level settings. This is optional; you can open or change a project any time."
  />

  <div class="flex flex-col gap-2">
    {#if project.recents.length > 0}
      <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Recent projects</h3>
      <ul class="flex max-h-64 flex-col gap-2 overflow-y-auto" aria-label="Recent projects">
        {#each project.recents as recent (recent.path)}
          <li>
            <ProjectRow project={recent} active={project.active?.path === recent.path} onselect={() => void project.open(recent.path)} />
          </li>
        {/each}
      </ul>
    {:else}
      <p class="rounded-md border border-line bg-glass p-4 text-sm text-fg-2">No recent projects yet.</p>
    {/if}
  </div>

  <div>
    <Button onclick={() => void project.pick()}>
      {#snippet leading()}<FolderOpen size={16} aria-hidden="true" />{/snippet}
      Open folder…
    </Button>
  </div>
</div>

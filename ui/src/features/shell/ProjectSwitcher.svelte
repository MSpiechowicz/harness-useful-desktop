<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import X from '@lucide/svelte/icons/x';

  import Button from '$lib/components/Button.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import ProjectRow from '$lib/components/ProjectRow.svelte';
  import { project } from '$lib/stores/project.svelte';

  let open = $state(false);
</script>

<Popover bind:open label="Projects" kind="dialog" class="w-96">
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      aria-label={project.active ? `Project: ${project.active.name}. Switch project` : 'No project open. Switch project'}
      class="inline-flex h-8 max-w-56 items-center gap-2 rounded-full border border-line bg-surface-2 px-3 text-sm text-fg transition-colors duration-[var(--dur-fast)] hover:border-line-strong hover:bg-surface-3"
    >
      <Folder size={16} class="shrink-0 text-fg-3" aria-hidden="true" />
      <span class="truncate {project.active ? '' : 'text-fg-2'}">{project.active ? project.active.name : 'No project'}</span>
      <ChevronDown size={16} class="shrink-0 text-fg-3" aria-hidden="true" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="flex flex-col gap-3 p-1.5">
      <section class="flex flex-col gap-1.5" aria-label="Active project">
        <h3 class="px-1 text-xs font-medium tracking-wider text-fg-3 uppercase">Active project</h3>

        {#if project.active}
          <div class="flex items-center gap-2 rounded-md border border-line bg-surface-2 px-3 py-2">
            <div class="flex min-w-0 flex-1 flex-col">
              <span class="truncate text-base font-medium text-fg">{project.active.name}</span>
              <span class="truncate font-mono text-xs text-fg-3">{project.active.path}</span>
            </div>
            <Button size="sm" variant="dangerQuiet" data-menu-item onclick={() => void project.close().then(close)}>Close project</Button>
          </div>
        {:else}
          <p class="px-1 text-sm text-fg-2">No project open. Config and extensions show your user-level setup.</p>
        {/if}
      </section>

      {#if project.recents.length > 0}
        <section class="flex flex-col gap-1.5" aria-label="Recent projects">
          <h3 class="px-1 text-xs font-medium tracking-wider text-fg-3 uppercase">Recent</h3>

          <ul class="flex max-h-64 flex-col gap-1 overflow-y-auto">
            {#each project.recents as recent (recent.path)}
              <li class="flex items-center gap-1">
                <div class="min-w-0 flex-1">
                  <ProjectRow
                    project={recent}
                    active={project.active?.path === recent.path}
                    menuItem
                    onselect={() => void project.open(recent.path).then(close)}
                  />
                </div>
                <IconButton icon={X} label="Forget {recent.name}" size="sm" data-menu-item onclick={() => void project.forget(recent.path)} />
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <Button variant="secondary" data-menu-item onclick={() => void project.pick().then(close)}>
        {#snippet leading()}<FolderOpen size={16} aria-hidden="true" />{/snippet}
        Open folder…
      </Button>
    </div>
  {/snippet}
</Popover>

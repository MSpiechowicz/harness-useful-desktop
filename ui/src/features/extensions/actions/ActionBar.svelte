<script lang="ts">
  import Info from '@lucide/svelte/icons/info';

  import Button from '$lib/components/Button.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';

  import type { BarAction } from './action-model';
  import { actionRunner } from './action-runner.svelte';

  interface Props {
    actions: BarAction[];
  }

  let { actions }: Props = $props();
</script>

<section class="flex flex-col gap-2" aria-label="Actions">
  <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Actions</h3>

  <div class="flex flex-wrap gap-2">
    {#each actions as action (action.label)}
      <Button variant={action.danger ? 'dangerQuiet' : 'secondary'} size="sm" disabled={actionRunner.phase !== 'idle'} onclick={() => void actionRunner.begin(action.request)}>
        {#snippet leading()}<action.icon size={ICON.sm} aria-hidden="true" />{/snippet}
        {action.label}
      </Button>
    {/each}
  </div>

  <p class="flex items-center gap-2 text-xs text-fg-3">
    <Info size={ICON.sm} class="shrink-0 text-info" aria-hidden="true" />
    Each action shows its exact command and, where the CLI can, a preview before it runs.
  </p>
</section>

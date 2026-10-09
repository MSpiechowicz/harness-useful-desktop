<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Folder from '@lucide/svelte/icons/folder';

  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import { onboarding } from './onboarding.svelte';
  import StepHeader from './StepHeader.svelte';

  const driver = $derived(onboarding.driver ? harness.get(onboarding.driver) : undefined);
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8', WIDTH.narrow)}>
  <StepHeader eyebrow="All set" title="You’re ready to go" body="Here is what we’ll start with. Everything can be changed later from the top bar or Settings." />

  <dl class="grid grid-cols-[max-content_1fr] items-center gap-x-6 gap-y-4 rounded-lg border border-line bg-glass p-5 backdrop-blur-md">
    <dt class="text-sm text-fg-3">Daily driver</dt>
    <dd class="flex items-center gap-3 text-base text-fg">
      {#if driver}
        <HarnessBadge info={driver.info} size="sm" />
        <span>{driver.info.name}</span>
        {#if driver.detection.version}<span class="tabular text-sm text-fg-3">v{driver.detection.version}</span>{/if}
      {:else}
        <span class="text-fg-2">None yet. Install OMP or PI, then re-scan from Settings.</span>
      {/if}
    </dd>

    <dt class="text-sm text-fg-3">Project</dt>
    <dd class="flex items-center gap-3 text-base text-fg">
      {#if project.active}
        <Folder class="text-fg-3" size={16} aria-hidden="true" />
        <span>{project.active.name}</span>
      {:else}
        <span class="text-fg-2">No project open</span>
      {/if}
    </dd>
  </dl>

  <p class="inline-flex items-center gap-2 text-sm text-fg-2"><Check class="text-success" size={16} aria-hidden="true" />Nothing leaves this device.</p>
</div>

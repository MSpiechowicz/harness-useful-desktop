<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Folder from '@lucide/svelte/icons/folder';

  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import { onboarding } from './onboarding.svelte';
  import StepHeader from './StepHeader.svelte';

  const driver = $derived(onboarding.driver ? harness.get(onboarding.driver) : undefined);

  /** The first things worth knowing once the app opens. */
  const TIPS: { keys: string; title: string; body: string }[] = [
    { keys: 'mod+k', title: 'Command palette', body: 'Jump anywhere, run actions, find sessions (@) and files (#).' },
    { keys: 'mod+n', title: 'New chat', body: 'Start a chat with your daily driver in the open project.' },
    { keys: 'mod+3', title: 'Changes', body: 'Review the agent’s diff and send line comments back to it.' },
    { keys: 'shift+?', title: 'All shortcuts', body: 'Everything has a key; this shows the full list.' },
  ];
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader eyebrow="All set" title="You’re ready to go" body="Here is what we’ll start with. Everything can be changed later from the top bar or Settings." />

  <div class="grid grid-cols-2 items-start gap-4">
    <section class="flex flex-col gap-4 rounded-lg border border-line bg-glass p-5" aria-labelledby="setup-summary">
      <h3 id="setup-summary" class="text-sm font-medium tracking-wider text-fg-3 uppercase">Your setup</h3>
      <dl class="grid grid-cols-[max-content_1fr] items-center gap-x-6 gap-y-4">
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
      <p class="inline-flex items-center gap-2 border-t border-line pt-3 text-sm text-fg-2"><Check class="text-success" size={16} aria-hidden="true" />Nothing leaves this device.</p>
    </section>

    <section class="flex flex-col gap-3 rounded-lg border border-line bg-glass p-5" aria-labelledby="first-steps">
      <h3 id="first-steps" class="text-sm font-medium tracking-wider text-fg-3 uppercase">Where to find things</h3>
      <ul class="flex flex-col gap-3">
        {#each TIPS as tip (tip.keys)}
          <li class="grid grid-cols-[max-content_1fr] items-start gap-4">
            <span class="pt-0.5"><Kbd keys={tip.keys} uniform /></span>
            <div class="flex flex-col gap-0.5">
              <p class="text-sm font-medium text-fg">{tip.title}</p>
              <p class="text-sm text-fg-2">{tip.body}</p>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  </div>
</div>

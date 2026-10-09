<script lang="ts">
  import ChartNoAxesColumn from '@lucide/svelte/icons/chart-no-axes-column';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
  import { openExternal } from '$lib/ipc/actions';

  import InstallSteps from '../InstallSteps.svelte';

  interface Props {
    item: EcosystemItem | undefined;
    checking: boolean;
    onrecheck: () => void;
  }

  let { item, checking, onrecheck }: Props = $props();
</script>

<section aria-labelledby="dashboard-install-title" class="flex flex-col gap-5 rounded-lg border border-line bg-surface-1 p-6 shadow-raised">
  <header class="flex items-start gap-4">
    <span class="grid size-12 shrink-0 place-items-center rounded-lg border border-line-strong bg-accent-wash text-accent-text shadow-float">
      <ChartNoAxesColumn size={24} strokeWidth={1.7} aria-hidden="true" />
    </span>
    <div class="flex min-w-0 flex-col gap-1">
      <h2 id="dashboard-install-title" class="text-xl text-fg">See what your agents cost</h2>
      <p class="max-w-prose text-base text-fg-2">
        Install the Harness Dashboard and this page fills with your cost, tokens, plan limits, budgets and failed tool calls, read from a local process on this machine. Nothing is sent over the network.
      </p>
    </div>
  </header>

  {#if item && item.installSteps.length > 0}
    <InstallSteps steps={item.installSteps} />
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <Button variant="primary" loading={checking} onclick={onrecheck}>
      {#snippet leading()}<RotateCw size={ICON.sm} aria-hidden="true" />{/snippet}
      Check again
    </Button>
    {#if item}
      <Button variant="ghost" onclick={() => void openExternal(item.docsUrl)}>
        {#snippet trailing()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
        Read the docs
      </Button>
    {/if}
  </div>
</section>

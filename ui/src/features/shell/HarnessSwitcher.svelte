<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Settings2 from '@lucide/svelte/icons/settings-2';

  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';

  import { onboarding } from '../onboarding/onboarding.svelte';

  let open = $state(false);

  const active = $derived(harness.active);

  async function choose(id: Parameters<typeof harness.select>[0], installed: boolean, close: () => void): Promise<void> {
    if (!installed) {
      return;
    }

    close();
    await harness.select(id);
  }
</script>

<Popover bind:open label="Switch harness" kind="menu" class="w-72">
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      aria-label={active ? `Harness: ${active.info.name}. Switch harness` : 'Harness: none detected'}
      class="inline-flex h-8 items-center gap-2 rounded-full border border-line bg-surface-2 pr-2.5 pl-1.5 text-sm text-fg transition-colors duration-[var(--dur-fast)] hover:border-line-strong hover:bg-surface-3"
    >
      {#if active}
        <HarnessBadge info={active.info} size="sm" class="rounded-full" />
        <span class="font-medium">{active.info.shortName}</span>
        {#if active.detection.version}
          <span class="tabular text-fg-3">v{active.detection.version}</span>
        {/if}
      {:else}
        <StatusDot tone="neutral" class="ml-1.5" />
        <span class="text-fg-2">No harness</span>
      {/if}
      <ChevronDown size={14} class="text-fg-3" aria-hidden="true" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="flex flex-col gap-0.5">
      {#each harness.summaries as summary (summary.info.id)}
        {@const installed = summary.detection.installed}
        {@const current = active?.info.id === summary.info.id}
        <Tooltip text="{summary.info.name} is not installed" side="right" disabled={installed} class="flex">
          {#snippet children({ describedby })}
            <button
              type="button"
              role="menuitemradio"
              aria-checked={current}
              aria-disabled={!installed}
              aria-describedby={installed ? undefined : describedby}
              data-menu-item
              onclick={() => void choose(summary.info.id, installed, close)}
              class={cx(
                'flex w-full items-center gap-3 rounded-md px-2.5 py-2 text-left transition-colors duration-[var(--dur-fast)]',
                current ? 'bg-accent-wash' : 'hover:bg-hover',
                !installed && 'cursor-not-allowed opacity-60',
              )}
            >
              <HarnessBadge info={summary.info} size="sm" />
              <span class="flex min-w-0 flex-1 flex-col">
                <span class="text-base font-medium text-fg">{summary.info.name}</span>
                <span class="tabular text-xs text-fg-3">
                  {installed ? (summary.detection.version ? `v${summary.detection.version}` : 'Installed') : 'Not installed'}
                </span>
              </span>
              <StatusDot tone={installed ? 'success' : 'warning'} />
            </button>
          {/snippet}
        </Tooltip>
      {/each}
    </div>

    <div class="mt-1 border-t border-line pt-1">
      <button
        type="button"
        role="menuitem"
        data-menu-item
        onclick={() => {
          close();
          onboarding.restart('harnesses');
        }}
        class="flex w-full items-center gap-3 rounded-md px-2.5 py-2 text-left text-sm text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg"
      >
        <Settings2 size={16} aria-hidden="true" />
        Detect or set up harnesses…
      </button>
    </div>
  {/snippet}
</Popover>

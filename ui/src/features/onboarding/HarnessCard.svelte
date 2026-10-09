<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import type { VersionSupport } from '$lib/ipc/bindings/VersionSupport';
  import { openExternal } from '$lib/ipc/actions';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    summary: HarnessSummary;
    /** This harness is the chosen daily driver. */
    selected: boolean;
    onselect: () => void;
  }

  let { summary, selected, onselect }: Props = $props();

  const SUPPORT: Record<VersionSupport, { label: string; tone: 'success' | 'warning' | 'neutral'; note: string | null }> = {
    supported: { label: 'Supported', tone: 'success', note: null },
    untested: {
      label: 'Untested version',
      tone: 'warning',
      note: 'This version is newer than the ones Harness Desktop was tested with. It should work, but some views may be incomplete.',
    },
    unknown: { label: 'Version unknown', tone: 'neutral', note: 'The version could not be read, so compatibility is unknown.' },
  };

  const detection = $derived(summary.detection);
  const support = $derived(SUPPORT[detection.versionSupport]);
  const label = $derived(summary.info.name);
</script>

<article
  aria-label={label}
  class={cx(
    'flex flex-col gap-4 rounded-lg border bg-glass p-5 shadow-float backdrop-blur-md transition-colors duration-[var(--dur-base)]',
    selected ? 'border-accent/50 shadow-glow' : 'border-line',
  )}
>
  <header class="flex items-start gap-3">
    <HarnessBadge info={summary.info} size="lg" />

    <h3 class="min-w-0 flex-1 self-center text-xl text-fg">{summary.info.name}</h3>

    <!-- The radio is its own control; the actions below stay outside it. -->
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      aria-disabled={!detection.installed}
      aria-label="Use {label} as daily driver"
      onclick={() => detection.installed && onselect()}
      class={cx(
        'flex h-8 shrink-0 items-center gap-2 rounded-full border px-3 whitespace-nowrap text-sm font-medium transition-colors duration-[var(--dur-fast)]',
        selected ? 'border-accent/50 bg-accent-wash text-fg' : 'border-line-strong text-fg-2',
        detection.installed ? 'hover:bg-hover' : 'cursor-not-allowed opacity-50',
      )}
    >
      <span class={cx('grid size-4 place-items-center rounded-full border', selected ? 'border-accent' : 'border-fg-3')} aria-hidden="true">
        {#if selected}<span class="size-2 rounded-full bg-accent"></span>{/if}
      </span>
      {selected ? 'Daily driver' : 'Use as daily driver'}
    </button>
  </header>

  <p class="text-sm text-fg-2">{summary.info.description}</p>

  {#if detection.installed}
    <div class="flex flex-wrap items-center gap-2">
      <Badge tone="success"><StatusDot tone="success" class="bg-current" />Installed</Badge>
      {#if detection.version}
        <Badge class="tabular">v{detection.version}</Badge>
      {/if}
      <Badge tone={support.tone}>{support.label}</Badge>
    </div>

    {#if support.note}
      <p class="text-sm text-fg-2">{support.note}</p>
    {/if}

    <dl class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1.5 text-sm">
      <dt class="text-fg-3">Binary</dt>
      <dd class="min-w-0 truncate font-mono text-fg-2" title={detection.binaryPath ?? undefined}>{detection.binaryPath}</dd>
    </dl>
  {:else}
    <div class="flex flex-wrap items-center gap-2">
      <Badge tone="warning"><StatusDot tone="warning" class="bg-current" />Not found</Badge>
    </div>

    {#if detection.installHint}
      <div class="flex flex-col gap-2">
        <p class="text-sm text-fg-2">Install it with:</p>
        <div class="flex items-center gap-2 rounded-md border border-line bg-surface-1 py-1 pr-1 pl-3">
          <code class="min-w-0 flex-1 py-1.5 font-mono text-sm break-all text-fg">{detection.installHint.command}</code>
          <CopyButton text={detection.installHint.command} label="Copy install command" />
        </div>
      </div>
    {/if}

    {#if detection.error}
      <p class="text-sm text-fg-3">{detection.error}</p>
    {/if}

    <div class="flex flex-wrap gap-2">
      {#if detection.installHint}
        {@const docsUrl = detection.installHint.docsUrl}
        <Button size="sm" onclick={() => void openExternal(docsUrl)}>
          {#snippet leading()}<ExternalLink size={14} aria-hidden="true" />{/snippet}
          Open install docs
        </Button>
      {/if}
      <Button size="sm" onclick={() => void harness.pickBinary(summary.info.id)}>
        {#snippet leading()}<FolderOpen size={14} aria-hidden="true" />{/snippet}
        Choose binary…
      </Button>
    </div>
  {/if}

  <dl class="grid grid-cols-[max-content_1fr] items-center gap-x-4 gap-y-1.5 border-t border-line pt-3 text-sm">
    <dt class="text-fg-3">Config</dt>
    <dd class="flex min-w-0 items-center gap-2">
      <span class="min-w-0 truncate font-mono text-fg-2" title={detection.configDir ?? undefined}>{detection.configDir ?? 'unknown'}</span>
      <Badge tone={detection.configDirExists ? 'success' : 'neutral'}>{detection.configDirExists ? 'Found' : 'Not created yet'}</Badge>
    </dd>
  </dl>
</article>

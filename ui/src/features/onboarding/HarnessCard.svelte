<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
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

  const SUPPORT = $derived<Record<VersionSupport, { label: string; tone: 'success' | 'warning' | 'neutral'; note: string | null }>>({
    supported: { label: t('onboarding.card.supported'), tone: 'success', note: null },
    untested: { label: t('onboarding.card.untested'), tone: 'warning', note: t('onboarding.card.untested.note') },
    unknown: { label: t('onboarding.card.unknownVersion'), tone: 'neutral', note: t('onboarding.card.unknownVersion.note') },
  });

  const detection = $derived(summary.detection);
  const support = $derived(SUPPORT[detection.versionSupport]);
  const label = $derived(summary.info.name);
</script>

<article
  aria-label={label}
  class={cx(
    'flex flex-col gap-4 rounded-lg border bg-glass p-5 shadow-float transition-colors duration-[var(--dur-base)] [@media(max-height:720px)]:gap-3 [@media(max-height:720px)]:p-4',
    selected ? 'edge-accent shadow-glow' : 'border-line',
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
      aria-label={t('onboarding.card.use', { name: label })}
      onclick={() => detection.installed && onselect()}
      class={cx(
        'flex h-8 shrink-0 items-center gap-2 rounded-full border px-3 whitespace-nowrap text-sm font-medium transition-colors duration-[var(--dur-fast)]',
        selected ? 'edge-accent bg-accent-wash text-fg' : 'border-line-strong text-fg-2',
        detection.installed ? 'hover:bg-hover active:bg-press' : 'cursor-not-allowed opacity-50',
      )}
    >
      <!-- One SVG, two circles on the same centre: concentric and crisp even on a half-pixel position. -->
      <svg class={cx('size-4 shrink-0', selected ? 'text-accent' : 'text-fg-3')} viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="7" fill="none" stroke="currentColor" stroke-width="1.5" />
        {#if selected}<circle cx="8" cy="8" r="3.5" fill="currentColor" />{/if}
      </svg>
      {selected ? t('onboarding.card.driver') : t('onboarding.card.useAs')}
    </button>
  </header>

  <p class="text-sm text-fg-2">{summary.info.description}</p>

  {#if detection.installed}
    <div class="flex flex-wrap items-center gap-2">
      <Badge tone="success"><StatusDot tone="success" />{t('onboarding.card.installed')}</Badge>
      {#if detection.version}
        <Badge class="tabular">v{detection.version}</Badge>
      {/if}
      <Badge tone={support.tone}>{support.label}</Badge>
    </div>

    {#if support.note}
      <p class="text-sm text-fg-2">{support.note}</p>
    {/if}

    <dl class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1.5 text-sm">
      <dt class="text-fg-3">{t('onboarding.card.binary')}</dt>
      <dd class="min-w-0 truncate font-mono text-fg-2" title={detection.binaryPath ?? undefined}>{detection.binaryPath}</dd>
    </dl>
  {:else}
    <div class="flex flex-wrap items-center gap-2">
      <Badge tone="warning"><StatusDot tone="warning" />{t('onboarding.card.notFound')}</Badge>
    </div>

    {#if detection.installHint}
      <div class="flex flex-col gap-2">
        <p class="text-sm text-fg-2">{t('onboarding.card.installWith')}</p>
        <div class="flex items-center gap-2 rounded-md border border-line bg-surface-1 py-1 pr-1 pl-3">
          <code class="min-w-0 flex-1 py-1.5 font-mono text-sm break-all text-fg">{detection.installHint.command}</code>
          <CopyButton text={detection.installHint.command} label={t('onboarding.card.copyInstall')} />
        </div>
      </div>
    {/if}

    {#if detection.error}
      <p class="text-sm text-fg-3">{detection.error}</p>
    {/if}

    <!-- An even row of equal-width actions, not a ragged wrap. -->
    <div class={cx('grid gap-2', detection.installHint ? 'grid-cols-2' : 'grid-cols-1')}>
      {#if detection.installHint}
        {@const docsUrl = detection.installHint.docsUrl}
        <Button size="sm" class="w-full" onclick={() => void openExternal(docsUrl)}>
          {#snippet leading()}<ExternalLink size={16} aria-hidden="true" />{/snippet}
          {t('onboarding.card.installDocs')}
        </Button>
      {/if}
      <Button size="sm" class="w-full" onclick={() => void harness.pickBinary(summary.info.id)}>
        {#snippet leading()}<FolderOpen size={16} aria-hidden="true" />{/snippet}
        {t('onboarding.card.chooseBinary')}
      </Button>
    </div>
  {/if}

  <dl class="grid grid-cols-[max-content_1fr] items-center gap-x-4 gap-y-1.5 border-t border-line pt-3 text-sm">
    <dt class="text-fg-3">{t('onboarding.card.config')}</dt>
    <dd class="flex min-w-0 items-center gap-2">
      <span class="min-w-0 truncate font-mono text-fg-2" title={detection.configDir ?? undefined}>{detection.configDir ?? t('onboarding.card.unknown')}</span>
      <Badge tone={detection.configDirExists ? 'success' : 'neutral'}>{detection.configDirExists ? t('onboarding.card.found') : t('onboarding.card.notCreated')}</Badge>
    </dd>
  </dl>
</article>

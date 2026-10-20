<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import FolderOpen from '@lucide/svelte/icons/folder-open';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { DURATION, rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import type { VersionSupport } from '$lib/ipc/bindings/VersionSupport';
  import { openExternal } from '$lib/ipc/actions';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';

  import { PICKER } from './layout';

  interface Props {
    summary: HarnessSummary;
    /** This harness is the chosen daily driver. */
    selected: boolean;
    onselect: () => void;
    id: string;
  }

  let { summary, selected, onselect, id }: Props = $props();

  const SUPPORT = $derived<Record<VersionSupport, { label: string; tone: 'success' | 'warning' | 'neutral'; note: string }>>({
    supported: { label: t('onboarding.card.supported'), tone: 'success', note: t('onboarding.card.supported.note') },
    untested: { label: t('onboarding.card.untested'), tone: 'warning', note: t('onboarding.card.untested.note') },
    unknown: { label: t('onboarding.card.unknownVersion'), tone: 'neutral', note: t('onboarding.card.unknownVersion.note') },
  });

  const detection = $derived(summary.detection);
  const support = $derived(SUPPORT[detection.versionSupport]);
  const label = $derived(summary.info.name);
  /** Detected and shown, but not offered yet: the app cannot chat with it. */
  const soon = $derived(!summary.capabilities.chat);
</script>

<!--
  The same rows, in this order, whatever the harness: header, extra facts, then its binary (or how to get one) and
  its config. The panel keeps a minimum height, so the footer below does not move from one harness to the next, and the
  spare room is at the bottom. A harness the app cannot chat with yet keeps the rows, greyed out, with "Coming soon" in place of the choice.
-->
<article {id} aria-label={label} class={cx('flex flex-col rounded-lg border-card bg-glass p-6 shadow-float transition-colors duration-[var(--dur-base)]', PICKER.detailMinHeight, selected ? 'edge-accent shadow-glow' : 'border-line')}>
  <!-- The content of another harness fades in over the same panel. -->
  {#key summary.info.id}
    <div class={cx('flex flex-1 flex-col gap-5', soon && 'opacity-60 grayscale')} in:rise={{ y: 6, duration: DURATION.base }}>
      <!-- The choice drops under the name when the panel is too narrow for both. -->
      <header class="flex flex-wrap items-start gap-x-3 gap-y-3">
        <HarnessBadge info={summary.info} size="lg" />

        <h3 class="min-w-24 flex-1 self-center text-xl text-fg">{summary.info.name}</h3>

        {#if soon}
          <Badge size="lg" class="h-control shrink-0">{t('onboarding.card.comingSoon')}</Badge>
        {:else}
          <div role="radiogroup" aria-label={t('onboarding.harnesses.group')} class="shrink-0">
            <button
              type="button"
              role="radio"
              aria-checked={selected}
              aria-disabled={!detection.installed}
              aria-label={t('onboarding.card.use', { name: label })}
              onclick={() => detection.installed && onselect()}
              class={cx(
                'flex h-control items-center gap-2 rounded-full border-card px-3 whitespace-nowrap text-sm font-medium transition-colors duration-[var(--dur-fast)]',
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
          </div>
        {/if}
      </header>

      <!-- The list row already says whether it is installed: only what the scan knows beyond that is shown here. -->
      {#if detection.installed}
        <div class="flex flex-wrap content-start items-center gap-2">
          {#if detection.version}
            <Badge size="lg" class="tabular">v{detection.version}</Badge>
          {/if}
          <!-- The badge carries the version note as a tooltip, so the panel stays short. -->
          <span class="inline-flex" title={support.note}>
            <Badge size="lg" tone={support.tone}>{support.label}</Badge>
          </span>
        </div>
      {/if}

      {#if !detection.installed && !soon}
        <!-- How to get a missing harness. -->
        <div class="flex flex-col gap-2">
          {#if detection.installHint}
            <p class="text-sm text-fg-2">{t('onboarding.card.installWith')}</p>
            <div class="flex items-center gap-2 rounded-md border border-line bg-surface-1 py-1 pr-1 pl-3">
              <code class="min-w-0 flex-1 py-1.5 font-mono text-sm [overflow-wrap:anywhere] text-fg">{detection.installHint.command}</code>
              <CopyButton text={detection.installHint.command} label={t('onboarding.card.copyInstall')} />
            </div>
          {/if}

          {#if detection.error}
            <p class="text-sm text-fg-3">{detection.error}</p>
          {/if}
        </div>

        <!-- An even row of equal-width actions, not a ragged wrap. -->
        <div class="flex flex-wrap items-center gap-2">
          {#if detection.installHint}
            {@const docsUrl = detection.installHint.docsUrl}
            <Button size="sm" class="min-w-36 flex-1" onclick={() => void openExternal(docsUrl)}>
              {#snippet leading()}<ExternalLink size={16} aria-hidden="true" />{/snippet}
              {t('onboarding.card.installDocs')}
            </Button>
          {/if}
          <Button size="sm" class="min-w-36 flex-1" onclick={() => void harness.pickBinary(summary.info.id)}>
            {#snippet leading()}<FolderOpen size={16} aria-hidden="true" />{/snippet}
            {t('onboarding.card.chooseBinary')}
          </Button>
        </div>
      {/if}

      <!-- Where it lives: the binary (once it is installed) and the config follow each other, whatever the harness. -->
      <dl class="grid grid-cols-[max-content_1fr] items-center gap-x-4 gap-y-3 text-sm">
        {#if detection.installed}
          <dt class="text-fg-3">{t('onboarding.card.binary')}</dt>
          <dd class="min-w-0 truncate font-mono text-fg-2" title={detection.binaryPath ?? undefined}>{detection.binaryPath ?? t('onboarding.card.unknown')}</dd>
        {/if}

        <dt class="text-fg-3">{t('onboarding.card.config')}</dt>
        <dd class="flex min-w-0 items-center gap-2">
          <span class="min-w-0 truncate font-mono text-fg-2" title={detection.configDir ?? undefined}>{detection.configDir ?? t('onboarding.card.unknown')}</span>
          <Badge size="lg" tone={detection.configDirExists ? 'success' : 'neutral'}>{detection.configDirExists ? t('onboarding.card.found') : t('onboarding.card.notCreated')}</Badge>
        </dd>
      </dl>
    </div>
  {/key}
</article>

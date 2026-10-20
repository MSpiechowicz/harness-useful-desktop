<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  interface Props {
    summaries: HarnessSummary[];
    /** The harness whose details are open. */
    focused: HarnessId;
    /** The chosen daily driver. */
    driver: HarnessId | null;
    /** The id of the panel the list controls. */
    panel: string;
    onfocus: (id: HarnessId) => void;
  }

  let { summaries, focused, driver, panel, onfocus }: Props = $props();

  let list = $state<HTMLElement>();

  /** One chip per row, from the most telling state to the least: the choice, what is not offered yet, then what the scan found. */
  function chipOf(summary: HarnessSummary): { label: string; tone: 'success' | 'warning' | 'neutral' } {
    if (summary.info.id === driver) {
      return { label: t('onboarding.card.driver'), tone: 'success' };
    }

    if (!summary.capabilities.chat) {
      return { label: t('onboarding.card.comingSoon'), tone: 'neutral' };
    }

    if (summary.detection.installed) {
      return { label: t('onboarding.card.installed'), tone: 'neutral' };
    }

    return { label: t('onboarding.card.notFound'), tone: 'warning' };
  }

  // Arrow keys walk the list and open each harness on the way, like a native list box.
  function onKeydown(event: KeyboardEvent): void {
    const current = summaries.findIndex((summary) => summary.info.id === focused);
    const next = summaries.length === 0 ? null : rovingIndex(event.key, current, summaries.length, 'vertical');
    if (next === null) {
      return;
    }

    event.preventDefault();

    const id = summaries[next].info.id;
    onfocus(id);
    list?.querySelector<HTMLElement>(`[data-harness="${id}"]`)?.focus();
  }
</script>

<!-- Every row has this shape: mark, name, and one chip at the right edge, centred on the row's height. -->
<div bind:this={list} role="listbox" aria-label={t('onboarding.harnesses.list')} tabindex="-1" class="flex flex-col gap-2 outline-none" onkeydown={onKeydown}>
  {#each summaries as summary (summary.info.id)}
    {@const id = summary.info.id}
    {@const chip = chipOf(summary)}
    {@const open = id === focused}

    <button
      type="button"
      role="option"
      aria-selected={open}
      aria-controls={panel}
      data-harness={id}
      tabindex={open ? 0 : -1}
      onclick={() => onfocus(id)}
      class={cx(
        'grid h-16 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 rounded-lg border-card px-3 text-left transition-colors duration-[var(--dur-fast)]',
        open ? 'edge-accent bg-accent-wash' : 'border-line bg-glass hover:bg-hover active:bg-press',
        !summary.capabilities.chat && 'opacity-60 grayscale',
      )}
    >
      <HarnessBadge info={summary.info} size="sm" />
      <span class="min-w-0 truncate text-base font-medium text-fg">{summary.info.name}</span>
      <Badge tone={chip.tone}>{chip.label}</Badge>
    </button>
  {/each}
</div>

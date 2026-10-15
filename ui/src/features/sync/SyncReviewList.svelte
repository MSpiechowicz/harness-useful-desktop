<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import type { SyncImportSection } from '$lib/ipc/bindings/SyncImportSection';
  import { harness } from '$lib/stores/harness.svelte';
  import { uid } from '$lib/utils/ids';

  import SyncGroupHead from './SyncGroupHead.svelte';
  import SyncItemRow from './SyncItemRow.svelte';
  import { madeLine } from './sync-model';
  import type { ImportController } from './sync-import-state.svelte';

  interface Props {
    flow: ImportController;
  }

  let { flow }: Props = $props();

  const plan = $derived(flow.plan);
  const made = $derived(plan ? madeLine(plan.app, plan.createdAt) : null);
  const base = uid('sync-review');
  const nameOf = (id: HarnessId) => harness.get(id)?.info.name ?? id.toUpperCase();

  /** How many of a harness's items are ticked, for the line under its name. */
  function tally(section: SyncImportSection): string | undefined {
    const selectable = section.items.filter((item) => item.selectable);

    if (selectable.length === 0) {
      return undefined;
    }

    return t('sync.review.selected', { count: selectable.filter((item) => flow.isSelected(item.id)).length, total: selectable.length });
  }
</script>

{#if plan}
  <div class="flex flex-col gap-6">
    {#if made}
      <p class="text-sm text-fg-3">{made}</p>
    {/if}

    {#if plan.sections.length === 0}
      <p class="text-base text-fg-2" role="status">{t('sync.review.empty')}</p>
    {/if}

    {#each plan.sections as section (section.harness)}
      <section aria-labelledby="{base}-{section.harness}">
        <SyncGroupHead
          id="{base}-{section.harness}"
          harness={section.harness}
          meta={tally(section)}
          all={section.items.some((item) => item.selectable)
            ? { checked: section.items.filter((item) => item.selectable).every((item) => flow.isSelected(item.id)), onchange: (on) => flow.toggleMany(section.items, on) }
            : undefined}
          chip={section.installed ? undefined : { tone: 'neutral', label: t('sync.review.notInstalled') }}
        />

        <ul>
          {#each section.items as item (item.id)}
            <SyncItemRow {item} {flow} disabled={flow.stage !== 'review'} />
          {/each}
        </ul>
      </section>
    {/each}

    {#if plan.unsupported.length > 0 || plan.invalidCount > 0}
      <section class="flex flex-col gap-1.5 border-t border-line pt-4" aria-label={t('sync.review.skipped.title')}>
        <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('sync.review.skipped.title')}</h3>

        <ul class="flex flex-col gap-1 text-sm text-fg-2">
          {#each plan.unsupported as entry, index (index)}
            <li>
              {#if entry.harness === null}
                {t('sync.review.unsupportedHarness', { name: entry.name, count: entry.itemCount })}
              {:else}
                {t('sync.review.unsupportedKind', { harness: nameOf(entry.harness), name: entry.name, count: entry.itemCount })}
              {/if}
            </li>
          {/each}

          {#if plan.invalidCount > 0}
            <li>{t('sync.review.invalid', { count: plan.invalidCount })}</li>
          {/if}
        </ul>
      </section>
    {/if}
  </div>
{/if}

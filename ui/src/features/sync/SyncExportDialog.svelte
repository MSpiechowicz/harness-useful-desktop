<script lang="ts">
  import { untrack } from 'svelte';

  import Check from '@lucide/svelte/icons/check';
  import FileDown from '@lucide/svelte/icons/file-down';
  import PackageOpen from '@lucide/svelte/icons/package-open';

  import Button from '$lib/components/Button.svelte';
  import Checkbox from '$lib/components/Checkbox.svelte';
  import CopyButton from '$lib/components/CopyButton.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { ipc } from '$lib/ipc/client';
  import { harness } from '$lib/stores/harness.svelte';
  import { uid } from '$lib/utils/ids';

  import SyncGroupHead from './SyncGroupHead.svelte';
  import SyncNotices from './SyncNotices.svelte';
  import SyncRow from './SyncRow.svelte';
  import { ExportController } from './sync-export-state.svelte';
  import { excludedText, itemLabel, showsVersion, sizeText } from './sync-model';

  interface Props {
    open: boolean;
  }

  let { open = $bindable(false) }: Props = $props();

  const flow = new ExportController(ipc);
  const base = uid('sync-export');
  const nameOf = (id: HarnessId) => harness.get(id)?.info.name ?? id.toUpperCase();

  // Opening the dialog reads the machine afresh.
  $effect(() => {
    if (open) {
      untrack(() => void flow.open());
    }
  });

  const chosen = $derived(flow.selectedIds.length);
  const excluded = $derived(flow.preview?.excluded ?? []);
</script>

<Dialog bind:open size="xl" title={t('sync.exportDialog.title')} description={t('sync.exportDialog.description')}>
  <div class="flex flex-col gap-6">
    {#if flow.load === 'loading' || flow.load === 'idle'}
      <div class="flex flex-col gap-3" aria-hidden="true">
        <Skeleton height="2.25rem" />
        <Skeleton height="2.25rem" />
        <Skeleton height="2.25rem" />
      </div>
    {:else if flow.load === 'error'}
      <p class="text-base text-danger" role="alert">{t('sync.exportDialog.error', { reason: flow.error })}</p>
    {:else}
      {#if flow.empty}
        <EmptyState icon={PackageOpen} title={t('sync.exportDialog.empty.title')} body={t('sync.exportDialog.empty.body')} />
      {/if}

      {#if flow.code}
        <section class="flex flex-col gap-1.5" aria-label={t('sync.exportDialog.codeLabel')}>
          <div class="flex items-baseline justify-between gap-3">
            <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('sync.exportDialog.codeLabel')}</h3>
            <p class="text-xs text-fg-3">{t('sync.exportDialog.codeMeta', { items: flow.code.itemCount, size: sizeText(flow.code.bytes) })}</p>
          </div>

          <div class="flex items-center gap-1 rounded-md border border-line bg-surface-2 py-0.5 pr-0.5 pl-3">
            <input readonly value={flow.code.code} aria-label={t('sync.exportDialog.codeLabel')} class="h-control min-w-0 flex-1 truncate bg-transparent font-mono text-xs text-fg outline-none" />
            <CopyButton text={flow.code.code} label={t('components.copy.code')} control />
          </div>

          <p class="text-xs text-fg-3">{t('sync.exportDialog.trust')}</p>
        </section>
      {/if}

      {#each flow.preview?.sections ?? [] as section (section.harness)}
        {@const ticked = section.items.filter((item) => flow.isSelected(item.id)).length}
        <section aria-labelledby="{base}-{section.harness}">
          <SyncGroupHead
            id="{base}-{section.harness}"
            harness={section.harness}
            meta={t('sync.exportDialog.selected', { count: ticked, total: section.items.length })}
            all={{ checked: ticked === section.items.length, onchange: (on) => flow.toggleMany(section.items.map((item) => item.id), on) }}
          />

          <ul>
            {#each section.items as item (item.id)}
              <SyncRow
                kind={item.kind}
                label={itemLabel(section.harness, item)}
                sub={[showsVersion(section.harness, item) ? t('sync.version', { version: item.version }) : null, item.detail].filter((part) => part !== null).join(' · ') || null}
                chip={item.runsProgram ? { tone: 'warning', label: t('sync.runs.chip') } : undefined}
                reserve
              >
                {#snippet check()}
                  <Checkbox label={t('sync.exportDialog.item', { name: itemLabel(section.harness, item) })} checked={flow.isSelected(item.id)} onchange={(on) => flow.toggle(item.id, on)} />
                {/snippet}

                <SyncNotices runsProgram={false} envKeys={item.envKeys} headerKeys={item.headerKeys} notes={item.notes} />
              </SyncRow>
            {/each}
          </ul>
        </section>
      {/each}

      {#if excluded.length > 0}
        <section aria-labelledby="{base}-excluded">
          <div class="flex flex-col gap-0.5 pb-2">
            <h3 id="{base}-excluded" class="text-base leading-5 font-medium text-fg">{t('sync.exportDialog.excluded.title')}</h3>
            <p class="text-xs leading-4 text-fg-3">{t('sync.exportDialog.excluded.body')}</p>
          </div>

          <ul>
            {#each excluded as entry, index (index)}
              <SyncRow kind={entry.kind} label={entry.label} sub={nameOf(entry.harness)} chip={{ tone: 'warning', label: excludedText(entry.reason) }} dim />
            {/each}
          </ul>
        </section>
      {/if}
    {/if}
  </div>

  {#snippet footer()}
    <div class="flex min-w-0 flex-1 items-center gap-3 text-sm">
      {#if flow.failure}
        <p class="truncate text-danger" role="alert" title={flow.failure}>{t('sync.exportDialog.failed', { reason: flow.failure })}</p>
      {:else if flow.savedTo}
        <p class="flex min-w-0 items-center gap-2 text-fg-2" role="status"><Check size={ICON.sm} class="shrink-0 text-success" aria-hidden="true" /><span class="truncate" title={flow.savedTo}>{t('sync.exportDialog.saved', { path: flow.savedTo })}</span></p>
      {:else if flow.saveCancelled}
        <p class="text-fg-3" role="status">{t('sync.exportDialog.saveCancelled')}</p>
      {/if}
    </div>

    <Button disabled={chosen === 0 || flow.busy} onclick={() => void flow.save()}>
      {#snippet leading()}<FileDown size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('sync.exportDialog.save')}
    </Button>

    <Button variant="primary" disabled={chosen === 0 || flow.busy} onclick={() => void flow.copy()}>
      {flow.copied ? t('sync.exportDialog.copied') : t('sync.exportDialog.copy')}
    </Button>
  {/snippet}
</Dialog>

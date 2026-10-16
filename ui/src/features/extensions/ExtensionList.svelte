<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import StatusDot from '$lib/components/StatusDot.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  import { rowInfo, type ExtItem } from './extension-items';
  import type { ExtGroup } from './extension-groups';
  import { groupFolds } from './group-folds.svelte';

  interface Props {
    groups: ExtGroup[];
    selected: ExtItem | undefined;
    /** A search is on: every group with a match is open, so the matches are never hidden behind a heading. */
    searching: boolean;
    onselect: (entry: ExtItem) => void;
  }

  let { groups, selected, searching, onselect }: Props = $props();
</script>

<!-- The Sessions and Configuration list: a heading per kind that folds, then one line per item. A long kind starts folded. -->
<nav aria-label={t('extensions.list.label')} class="flex min-h-0 flex-1 flex-col overflow-y-auto pt-1 pb-3">
  {#each groups as group (group.tab)}
    {@const open = searching || groupFolds.isOpen(group.tab, group.total)}

    <section aria-label={group.label}>
      <div class="px-3 pt-[14px] pb-[6px]">
        <button
          type="button"
          aria-expanded={open}
          class="flex h-[28px] w-full items-center gap-[10px] rounded-sm px-[12px] text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px]"
          onclick={() => groupFolds.toggle(group.tab, group.total)}
        >
          <span class="grid size-[16px] shrink-0 place-items-center"><ChevronRight size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} /></span>
          <span class="flex-1 truncate">{group.label}</span>
          <span class="tabular">{formatCount(group.items.length)}</span>
        </button>
      </div>

      {#each group.items as entry (entry.id)}
        {@const current = entry === selected}

        {#if open}
          {@const info = rowInfo(entry)}

          <div class="h-[40px] px-3 py-[2px]">
            <button
              type="button"
              title={entry.title}
              aria-current={current ? 'true' : undefined}
              onclick={() => onselect(entry)}
              class={cx(
                'flex size-full items-center gap-2.5 rounded-md border pr-2.5 pl-2.5 text-left transition-colors duration-[var(--dur-fast)]',
                current ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
              )}
            >
              {#if info.status}
                <StatusDot tone={info.status === 'on' ? 'success' : 'neutral'} />
              {/if}

              <span class="min-w-0 flex-1 truncate text-sm text-fg">{entry.title}</span>

              {#if info.update}
                <span class="h-4 shrink-0 rounded-full bg-info-wash px-1.5 text-[11px] leading-4 font-medium text-info">{t('extensions.row.update')}</span>
              {/if}
              {#if info.meta}
                <span class={cx('tabular shrink-0 text-xs', current ? 'text-fg-2' : 'text-fg-3')}>{info.meta}</span>
              {/if}
            </button>
          </div>
        {/if}
      {/each}
    </section>
  {/each}
</nav>

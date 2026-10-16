<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  import { folded } from './layer-folds.svelte';
  import { baseName, FORMAT_TILES, groupSections } from './sections';

  interface Props {
    sections: ConfigSection[];
    selectedId: string;
    onselect: (id: string) => void;
    /** While a search runs every layer stays open, so no match hides behind a fold. */
    searching?: boolean;
  }

  let { sections, selectedId, onselect, searching = false }: Props = $props();

  const groups = $derived(groupSections(sections));
</script>

<!-- The Sessions list: a heading per layer that folds, then one line per file. The file name is in the row's tooltip. -->
<nav aria-label={t('config.list.label')} class="flex min-h-0 flex-1 flex-col overflow-y-auto pt-1 pb-3">
  {#each groups as group (group.layer)}
    {@const open = searching || !folded.has(group.layer)}

    <section aria-label={t('config.list.layer', { layer: group.title })}>
      <div class="px-3 pt-[14px] pb-[6px]">
        <button
          type="button"
          aria-expanded={open}
          class="group/heading flex h-[28px] w-full items-center gap-[10px] rounded-sm px-[12px] text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px]"
          onclick={() => folded.toggle(group.layer)}
        >
          <span class="grid size-[16px] shrink-0 place-items-center"><ChevronRight size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} /></span>
          <span class="flex-1 truncate">{group.title}</span>
          <span class="tabular">{formatCount(group.sections.length)}</span>
        </button>
      </div>

      {#each group.sections as section (section.id)}
        {@const selected = section.id === selectedId}

        {#if open}
          <div class="h-[40px] px-3 py-[2px]">
            <button
              type="button"
              title={baseName(section.source.path)}
              aria-current={selected ? 'true' : undefined}
              onclick={() => onselect(section.id)}
              class={cx(
                'flex size-full items-center gap-2.5 rounded-md border pr-2 pl-2.5 text-left transition-colors duration-[var(--dur-fast)]',
                selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
              )}
            >
              <span
                aria-hidden="true"
                class={cx(
                  'grid h-5 min-w-[48px] shrink-0 place-items-center rounded-sm px-1.5 text-[10px] leading-none font-semibold tracking-wide transition-colors duration-[var(--dur-fast)]',
                  selected ? 'bg-accent/18 text-accent-text' : 'bg-surface-3 text-fg-2',
                )}
              >{FORMAT_TILES[section.source.format]}</span>

              <span class="min-w-0 flex-1 truncate text-sm text-fg">{section.title}</span>

              {#if section.error}
                <Badge tone="danger">{t('config.list.error')}</Badge>
              {:else if !section.source.exists}
                <Badge tone="neutral">{t('config.list.notFound')}</Badge>
              {:else if section.source.profile && !section.title.includes(section.source.profile)}
                <Badge tone="info" class="max-w-[40%] truncate">{section.source.profile}</Badge>
              {/if}
            </button>
          </div>
        {/if}
      {/each}
    </section>
  {/each}
  {#if searching && groups.length === 0}
    <p class="px-4 py-6 text-center text-sm text-fg-3" role="status">{t('config.list.noMatch')}</p>
  {/if}
</nav>

<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import Card from '$lib/components/Card.svelte';
  import ConfigTree from '$lib/components/ConfigTree.svelte';
  import ConfigValueView from '$lib/components/ConfigValueView.svelte';
  import HighlightedText from '$lib/components/HighlightedText.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import { matchIndices, normalizeNeedle } from '$lib/components/config-tree';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
  import type { ConfigFormat } from '$lib/ipc/bindings/ConfigFormat';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  import RolesTable from './RolesTable.svelte';
  import { buildGroups, GROUP_ORDER, type GroupId } from './structured';

  interface Props {
    fields: ConfigField[];
    /** Case-insensitive filter over keys and plain values (never secrets). */
    query: string;
    /** Accessible name of the view. */
    label: string;
    /** Which language the file is written in; the structure shown does not depend on it. */
    format: ConfigFormat;
    /** Out: open cards plus open rows of the trees inside them. */
    openCount?: number;
  }

  let { fields, query, label, openCount = $bindable(0) }: Props = $props();

  const GROUP_TITLES: Record<GroupId, () => string> = {
    modelRoles: () => t('components.configView.section.modelRoles'),
    appearance: () => t('components.configView.section.appearance'),
    behavior: () => t('components.configView.section.behavior'),
    memory: () => t('components.configView.section.memory'),
    developer: () => t('components.configView.section.developer'),
    other: () => t('components.configView.section.misc'),
  };

  let closed = $state.raw<ReadonlySet<GroupId>>(new Set());
  let treeOpen = $state<Partial<Record<GroupId, number>>>({});
  let trees = $state<Partial<Record<GroupId, ReturnType<typeof ConfigTree> | undefined>>>({});

  const needle = $derived(normalizeNeedle(query));
  const groups = $derived(buildGroups(fields, query));

  // While searching every card stays open, so no match hides behind a fold.
  const isOpen = (id: GroupId): boolean => needle !== '' || !closed.has(id);

  $effect(() => {
    openCount = groups.filter((group) => isOpen(group.id)).reduce((sum, group) => sum + 1 + (treeOpen[group.id] ?? 0), 0);
  });

  /** Opens or closes every card and every row of the trees inside them. */
  export function setAll(open: boolean): void {
    closed = open ? new Set() : new Set(GROUP_ORDER);

    for (const tree of Object.values(trees)) {
      tree?.setAll(open);
    }
  }

  function toggle(id: GroupId): void {
    const next = new Set(closed);

    if (!next.delete(id)) {
      next.add(id);
    }

    closed = next;
  }

  function highlight(text: string): number[] {
    return matchIndices(text, needle);
  }
</script>

{#if groups.length === 0}
  <p class="px-3 py-6 text-center text-sm text-fg-3" role="status">
    {fields.length === 0 ? t('components.config.none') : t('components.config.noMatch')}
  </p>
{:else}
  <!-- A file whose keys are all unfamiliar (an MCP list, say) is one plain tree: a lone "Other" heading would say nothing. -->
  {@const bare = groups.length === 1 && groups[0].id === 'other'}
  <div role="group" aria-label={label} class="flex flex-col gap-3">
    {#each groups as group (group.id)}
      {@const open = bare || isOpen(group.id)}
      {@const title = GROUP_TITLES[group.id]()}

      <Card as="section" padded={false} class="overflow-hidden">
        {#if !bare}
        <button
          type="button"
          aria-expanded={open}
          aria-controls="config-group-{group.id}"
          disabled={needle !== ''}
          class="group/heading flex h-[36px] w-full items-center gap-[10px] px-4 text-left text-xs font-medium tracking-wide text-fg-3 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px] disabled:hover:bg-transparent disabled:hover:text-fg-3"
          onclick={() => toggle(group.id)}
        >
          <span class="grid size-[16px] shrink-0 place-items-center"><ChevronRight size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} /></span>
          <span class="flex-1 truncate">{title}</span>
          <span class="tabular">{formatCount(group.count)}</span>
        </button>
        {/if}

        <!-- Hidden, not removed, when folded: the tree inside keeps what the person opened. -->
        <div id="config-group-{group.id}" role="region" aria-label={title} hidden={!open} class={cx('px-4 pb-3', bare && 'pt-3')}>
          {#if group.body.kind === 'roles'}
            <RolesTable rows={group.body.rows} label={title} {needle} />
          {:else}
            {#each group.body.scalars as field (field.key)}
              <div class="flex min-h-[44px] items-center gap-4 border-t border-line py-2 first:border-t-0">
                <span class="min-w-0 flex-1 truncate text-sm text-fg"><HighlightedText text={field.key} indices={highlight(field.key)} /></span>

                {#if field.value.kind === 'bool'}
                  <Switch checked={field.value.value} label={field.key} disabled />
                {:else if field.value.kind === 'text'}
                  <span class="max-w-[60%] truncate rounded-full bg-surface-3 px-[10px] py-[2px] font-mono text-xs text-success">
                    <HighlightedText text={field.value.value} indices={highlight(field.value.value)} />
                  </span>
                {:else}
                  <ConfigValueView value={field.value} class="text-sm" />
                {/if}
              </div>
            {/each}

            {#if group.body.nested.length > 0}
              <div class={cx(group.body.scalars.length > 0 && 'mt-1 border-t border-line pt-1')}>
                <ConfigTree
                  bind:this={trees[group.id]}
                  bind:openCount={() => treeOpen[group.id] ?? 0, (count) => (treeOpen[group.id] = count)}
                  fields={group.body.nested}
                  label={title}
                  {query}
                />
              </div>
            {/if}
          {/if}
        </div>
      </Card>
    {/each}
  </div>
{/if}

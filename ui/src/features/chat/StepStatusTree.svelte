<script lang="ts" module>
  import type { Component } from 'svelte';

  export interface TreeBadge {
    label: string;
    tone: 'info' | 'neutral';
    icon?: Component<{ size?: number; 'aria-hidden'?: 'true' }>;
  }

  export interface TreeRow {
    key: string;
    title: string;
    /** The line under the title. */
    detail?: string | null;
    /** A file name, not a sentence. */
    mono?: boolean;
    /** Read a step quieter: the reply did not use it. */
    quiet?: boolean;
    badges: TreeBadge[];
    /** Opens the row in its own view. */
    onopen?: () => void;
  }

  export interface TreeGroup {
    key: string;
    label: string;
    /** A path or the words of a search, not a sentence. */
    mono: boolean;
    rows: TreeRow[];
  }
</script>

<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    groups: TreeGroup[];
    /** The icon of a group and of a row. */
    groupIcon: Component<{ size?: number; 'aria-hidden'?: 'true' }>;
    rowIcon: Component<{ size?: number; class?: string; 'aria-hidden'?: 'true' }>;
  }

  let { groups, groupIcon: GroupIcon, rowIcon: RowIcon }: Props = $props();

  /** The most rows shown before "Show N more". */
  const COLLAPSED_ROWS = 8;

  let expanded = $state(false);

  const total = $derived(groups.reduce((sum, group) => sum + group.rows.length, 0));

  /** The groups cut to the rows that fit, so a group that lost all of them loses its heading too. */
  const shown = $derived.by(() => {
    if (expanded) {
      return groups;
    }

    let left = COLLAPSED_ROWS;

    return groups.flatMap((group) => {
      const rows = group.rows.slice(0, left);

      left -= rows.length;

      return rows.length > 0 ? [{ ...group, rows }] : [];
    });
  });
</script>

<div class="text-sm">
  {#each shown as group (group.key)}
    <p class={cx('flex items-center gap-1.5 pt-1.5 pb-1 text-fg-3', group.mono ? 'font-mono text-xs' : 'text-xs')}>
      <GroupIcon size={14} aria-hidden="true" />
      <span class="min-w-0 truncate" title={group.label}>{group.label}</span>
    </p>

    <ul class="ml-1.5 border-l border-line-strong">
      {#each group.rows as row (row.key)}
        <li class="grid min-h-8 grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 rounded-r-sm py-1 pr-1.5 pl-3.5 transition-colors duration-[var(--dur-fast)] hover:bg-hover">
          <div class="min-w-0">
            <!-- The icon shares the title's line, so it stays on it whatever the row holds under the title. -->
            <div class="flex h-5 items-center gap-1.5">
              <RowIcon size={14} aria-hidden="true" class={cx('shrink-0', row.quiet ? 'text-fg-3' : 'text-accent-text')} />
              {#if row.onopen}
                <button type="button" class={cx('min-w-0 truncate text-left hover:underline', row.quiet ? 'text-fg-3' : 'text-fg')} title={row.title} onclick={row.onopen}>{row.title}</button>
              {:else}
                <span class={cx('min-w-0 truncate', row.mono && 'font-mono', row.quiet ? 'text-fg-3' : 'text-fg')} title={row.title}>{row.title}</span>
              {/if}
            </div>
            {#if row.detail}<p class="truncate pl-5 text-xs text-fg-3" title={row.detail}>{row.detail}</p>{/if}
          </div>

          {#if row.badges.length > 0}
            <span class="flex items-center gap-1.5">
              {#each row.badges as badge (badge.label)}
                <Badge tone={badge.tone} size="row">
                  {#if badge.icon}<badge.icon size={14} aria-hidden="true" />{/if}{badge.label}
                </Badge>
              {/each}
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {/each}

  {#if total > COLLAPSED_ROWS}
    <button type="button" class="mt-1.5 ml-1.5 text-accent-text hover:underline" onclick={() => (expanded = !expanded)}>
      {expanded ? t('chat.stepStatus.fewer') : t('chat.stepStatus.more', { count: total - COLLAPSED_ROWS })}
    </button>
  {/if}
</div>

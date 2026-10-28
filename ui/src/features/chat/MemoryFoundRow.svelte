<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Eye from '@lucide/svelte/icons/eye';
  import Play from '@lucide/svelte/icons/play';
  import ThumbsUp from '@lucide/svelte/icons/thumbs-up';

  import Badge from '$lib/components/Badge.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { FoundRow, UnknownNote } from '$lib/chat/memory-tools';
  import type { RowUse } from '$lib/chat/memory-use';
  import { toolContext } from '$lib/chat/tool-context';
  import { t } from '$lib/i18n/i18n.svelte';
  import { perLocale } from '$lib/utils/format';
  import { cx } from '$lib/utils/cx';

  import NoteKindBadge from '../memory/NoteKindBadge.svelte';
  import { showNote, showScript } from '../memory/memory-open';
  import { languageLabel } from '../memory/script-format';

  interface Props {
    /** A note or script a search listed, or a note only its id is known of (no badge and no details, just `#<id>`). */
    row: FoundRow | UnknownNote;
    /** What the reply did with it; null for a row nothing is known of (or that is not marked yet). */
    use: RowUse | null;
    /** A row the reply did not use: its title reads a step quieter. */
    dim?: boolean;
  }

  let { row, use, dim = false }: Props = $props();

  const where = toolContext();

  const DAY = perLocale((l) => new Intl.DateTimeFormat(l, { day: 'numeric', month: 'short', timeZone: 'UTC' }));
  const DAY_YEAR = perLocale((l) => new Intl.DateTimeFormat(l, { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' }));

  /** "14 Oct", with the year once it is not this one. */
  function shortDate(day: string): string {
    const date = new Date(`${day}T00:00:00Z`);
    if (Number.isNaN(date.getTime())) {
      return day;
    }

    return (date.getUTCFullYear() === new Date().getUTCFullYear() ? DAY() : DAY_YEAR()).format(date);
  }

  /** A repository note says the folder it was written from, a global one says so; "Repository" only when the server named no folder. */
  function whereOf(note: { scope: 'repo' | 'global'; path: string | null }): string {
    if (note.scope === 'global') {
      return t('chat.memory.row.scope.global');
    }

    return note.path ?? t('chat.memory.row.scope.repo');
  }

  const meta = $derived.by(() => {
    if (row.type === 'unknown') {
      return '';
    }

    if (row.type === 'script') {
      return row.description;
    }

    return `${whereOf(row)} · ${shortDate(row.date)}`;
  });
  const title = $derived.by(() => {
    if (row.type === 'unknown') {
      return `#${row.id}`;
    }

    return row.type === 'note' ? row.title : row.name;
  });

  function show(): void {
    if (row.type === 'unknown') {
      showNote(row.id);
    } else if (row.type === 'note') {
      showNote(row.id, row.scope);
    } else {
      void showScript(row.id ?? row.name, where?.project ?? null);
    }
  }
</script>

<li class="grid grid-cols-[108px_minmax(0,1fr)_auto] items-center gap-x-3 px-[18px] py-2.5 transition-colors duration-[var(--dur-fast)] hover:bg-hover">
  <span class="justify-self-start">
    {#if row.type === 'unknown'}
      <!-- Nothing is known of the note but its id: no badge to show. -->
    {:else if row.type === 'note'}
      <NoteKindBadge kind={row.kind} size="lg" />
    {:else}
      <Badge size="lg">{languageLabel(row.language)}</Badge>
    {/if}
  </span>

  <div class="flex min-w-0 flex-col gap-0.5">
    <span class={cx('truncate text-sm', dim ? 'text-fg-2' : 'text-fg')}>{title}</span>
    {#if meta}<span class="truncate text-xs text-fg-3" title={meta}>{meta}</span>{/if}
    {#if use?.kind === 'useful' && use.reason}
      <span class="text-[12.5px] leading-snug text-fg-2">{t('chat.memory.row.reason', { reason: use.reason })}</span>
    {/if}
  </div>

  <div class="flex items-center gap-2">
    {#if use?.kind === 'useful'}
      <span class="inline-flex items-center gap-1.5 text-xs font-medium text-info"><ThumbsUp size={14} aria-hidden="true" />{t('chat.memory.row.useful')}</span>
    {:else if use?.kind === 'opened'}
      <span class="inline-flex items-center gap-1.5 text-xs text-fg-3"><Eye size={14} aria-hidden="true" />{t('chat.memory.row.opened')}</span>
    {:else if use?.kind === 'ran'}
      <span class="inline-flex items-center gap-1.5 text-xs text-fg-3"><Play size={14} aria-hidden="true" />{t('chat.memory.row.ran')}</span>
    {/if}

    <Tooltip text={t('chat.memory.row.show')}>
      {#snippet children({ describedby })}
        <button
          type="button"
          aria-label={t('chat.memory.row.show')}
          aria-describedby={describedby}
          class="inline-grid size-7 place-items-center rounded-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg active:bg-press"
          onclick={show}
        >
          <ExternalLink size={16} aria-hidden="true" />
        </button>
      {/snippet}
    </Tooltip>
  </div>
</li>

<script lang="ts">
  import ArrowDownLeft from '@lucide/svelte/icons/arrow-down-left';
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';

  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { NoteDetail } from '$lib/ipc/bindings/NoteDetail';
  import type { NoteSummary } from '$lib/ipc/bindings/NoteSummary';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';

  import BodyState from './BodyState.svelte';
  import HiddenText from './HiddenText.svelte';
  import NoteKindBadge from './NoteKindBadge.svelte';
  import { KIND_LABELS, noteFacts, noteScopeText } from './note-format';
  import PanelActions from './PanelActions.svelte';
  import { revealHidden, spelledOut } from './script-format';

  interface Props {
    open: boolean;
    /** Fills a column of the view, instead of sliding in over its edge. */
    inline: boolean;
    /** The note, as the list or the read of it gave it; null while a note that is not on this page is still being read. */
    note: NoteSummary | null;
    /** The note with its body and links; null until it was read. */
    detail: NoteDetail | null;
    status: ViewStatus;
    error: string | null;
    onretry: () => void;
    /** A linked note was chosen. */
    onopennote: (id: number) => void;
    ondelete: () => void;
    onclose: () => void;
  }

  let { open, inline, note, detail, status, error, onretry, onopennote, ondelete, onclose }: Props = $props();

  const subtitle = $derived(note ? `${t(KIND_LABELS[note.kind])} · ${noteScopeText(note)}` : '');
  const hidden = $derived(detail ? revealHidden(detail.note.body).count : 0);
  const groups = $derived(
    note
      ? [
          { label: t('memory.notes.detail.tags'), values: note.tags, mono: false },
          { label: t('memory.notes.detail.entities'), values: note.entities, mono: true },
        ].filter((group) => group.values.length > 0)
      : [],
  );
</script>

<SidePanel {open} {inline} title={note ? spelledOut(note.title) : ''} {subtitle} {onclose}>
  {#if note}
    <SectionCard flat heading={t('memory.notes.detail.content')}>
      <BodyState loaded={detail !== null} {status} {error} failed={t('memory.notes.body.failed')} loading={t('memory.notes.body.loading')} {onretry}>
        {#if hidden > 0}
          <p class="mb-3 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">{tn('memory.notes.body.hidden', hidden)}</p>
        {/if}
        <p class="text-sm leading-relaxed break-words whitespace-pre-wrap text-fg-2"><HiddenText text={detail?.note.body ?? ''} /></p>
      </BodyState>
    </SectionCard>

    <SectionCard flat heading={t('memory.notes.detail.tagsAndEntities')}>
      {#if groups.length === 0}
        <p class="text-sm text-fg-3">{t('memory.notes.detail.noTags')}</p>
      {:else}
        <div class="flex flex-col gap-3">
          {#each groups as group (group.label)}
            <div class="flex flex-col gap-1.5">
              <p class="text-xs text-fg-3">{group.label}</p>
              <ul class="flex flex-wrap gap-1.5">
                {#each group.values as value (value)}
                  <li class={group.mono ? 'rounded-sm bg-surface-3 px-2 py-0.5 font-mono text-xs text-fg-2' : 'rounded-full bg-surface-3 px-2.5 py-0.5 text-xs text-fg-2'}>{spelledOut(value)}</li>
                {/each}
              </ul>
            </div>
          {/each}
        </div>
      {/if}
    </SectionCard>

    <SectionCard flat heading={t('memory.notes.detail.links')}>
      {#if !detail}
        <Skeleton height="1.25rem" />
      {:else if detail.links.length === 0}
        <p class="text-sm text-fg-3">{t('memory.notes.detail.noLinks')}</p>
      {:else}
        <ul class="-mx-1.5 flex flex-col">
          {#each detail.links as link (`${link.id}:${link.direction}:${link.relation}`)}
            {@const Direction = link.direction === 'out' ? ArrowUpRight : ArrowDownLeft}

            <li>
              <button
                type="button"
                title={t(link.direction === 'out' ? 'memory.notes.link.out' : 'memory.notes.link.in')}
                class="flex h-10 w-full items-center gap-2.5 rounded-md px-1.5 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press"
                onclick={() => onopennote(link.id)}
              >
                <Direction size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
                <span class="min-w-0 flex-1 truncate text-sm text-fg">{spelledOut(link.title)}</span>
                <span class="shrink-0 text-xs text-fg-3">{link.relation.replaceAll('_', ' ')}</span>
                <NoteKindBadge kind={link.kind} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </SectionCard>

    <PanelActions deleteLabel={t('memory.panel.delete')} {ondelete} />

    <FactsCard flat heading={t('memory.panel.details')} rows={noteFacts(note)} />
  {/if}
</SidePanel>

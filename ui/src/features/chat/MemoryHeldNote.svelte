<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import Markdown from '$lib/components/Markdown.svelte';
  import { isLong, type HeldNoteView } from '$lib/chat/memory-held';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import HiddenText from '../memory/HiddenText.svelte';
  import { lineDiff } from '../memory/line-diff';
  import NoteKindBadge from '../memory/NoteKindBadge.svelte';
  import ScriptDiff from '../memory/ScriptDiff.svelte';
  import { revealHidden, spelledOut } from '../memory/script-format';
  import MemoryFolded from './MemoryFolded.svelte';
  import MemoryHeldMeta from './MemoryHeldMeta.svelte';

  interface Props {
    view: HeldNoteView;
  }

  let { view }: Props = $props();

  const note = $derived(view.note);
  const current = $derived(view.current);
  const hidden = $derived(revealHidden(note.body).count);
  const diff = $derived(current ? lineDiff(current.body, note.body) : null);
  const renamedFrom = $derived(current && current.title !== note.title ? current.title : null);
  const meta = $derived([
    { label: t('memory.guards.waiting.tags'), values: note.tags.map((tag) => `#${tag}`), mono: false },
    { label: t('memory.guards.waiting.about'), values: note.entities, mono: true },
  ]);
</script>

<div class="flex flex-wrap items-center gap-2">
  <NoteKindBadge kind={note.kind} size="lg" />
  <Badge size="lg">{note.scope === 'repo' ? t('memory.guards.waiting.scope.repo') : t('memory.guards.waiting.scope.global')}</Badge>
  <Badge size="lg" tone={view.noteId === null ? 'neutral' : 'info'}>{view.noteId === null ? t('memory.guards.waiting.note.new') : t('memory.guards.waiting.note.update', { id: view.noteId })}</Badge>
</div>

<h4 class="text-base font-semibold break-words text-fg">{spelledOut(note.title)}</h4>
{#if renamedFrom !== null}
  <p class="-mt-2 text-xs text-fg-3">{t('memory.guards.waiting.note.was', { title: spelledOut(renamedFrom) })}</p>
{/if}

{#if hidden > 0}
  <p class="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">{tn('memory.notes.body.hidden', hidden)}</p>
{/if}

<MemoryFolded long={isLong(note.body)}>
  {#if hidden > 0}
    <p class="text-sm leading-relaxed break-words whitespace-pre-wrap text-fg"><HiddenText text={note.body} /></p>
  {:else}
    <Markdown source={note.body} />
  {/if}
</MemoryFolded>

{#if diff && diff.rows.length > 0}
  <div class="flex flex-col gap-1.5">
    <p class="tabular flex items-center gap-2 text-xs text-fg-3">
      {t('memory.guards.waiting.note.changes')}
      <span class="font-mono"><span class="text-[color:var(--shiki-token-inserted)]">+{diff.added}</span> <span class="text-[color:var(--shiki-token-deleted)]">−{diff.removed}</span></span>
    </p>
    <ScriptDiff {diff} label={t('memory.guards.waiting.note.diffLabel')} />
  </div>
{/if}

<MemoryHeldMeta rows={meta} />

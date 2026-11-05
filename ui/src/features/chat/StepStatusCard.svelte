<script lang="ts">
  import BookOpenText from '@lucide/svelte/icons/book-open-text';
  import CircleDot from '@lucide/svelte/icons/circle-dot';
  import ClipboardList from '@lucide/svelte/icons/clipboard-list';
  import Eye from '@lucide/svelte/icons/eye';
  import File from '@lucide/svelte/icons/file';
  import FileCode from '@lucide/svelte/icons/file-code';
  import Folder from '@lucide/svelte/icons/folder';
  import Gauge from '@lucide/svelte/icons/gauge';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Layers from '@lucide/svelte/icons/layers';
  import NotebookText from '@lucide/svelte/icons/notebook-text';
  import Search from '@lucide/svelte/icons/search';
  import SearchX from '@lucide/svelte/icons/search-x';
  import ThumbsUp from '@lucide/svelte/icons/thumbs-up';
  import Waypoints from '@lucide/svelte/icons/waypoints';
  import type { Component } from 'svelte';

  import { type CodeLookup, readCodeGraph } from '$lib/chat/code-graph';
  import type { KnowledgeUseIndex } from '$lib/chat/knowledge-use';
  import type { MemoryUseIndex } from '$lib/chat/memory-use';
  import { checkedCounts, checkedDocs, checkedNotes, type CheckedGroup, replyOf } from '$lib/chat/reply-checks';
  import { readStepStatus, saysNone, type StepLevel } from '$lib/chat/step-status';
  import type { ToolCallState, TranscriptItem } from '$lib/chat/types';
  import type { RowUse } from '$lib/chat/use-index';
  import { t, tn, type MessageKey } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import { showNote } from '../memory/memory-open';

  import ChatRow from './ChatRow.svelte';
  import { showDocument } from './knowledge-open';
  import StatusPill from './StatusPill.svelte';
  import StepStatusRow from './StepStatusRow.svelte';
  import StepStatusTree, { type TreeBadge, type TreeGroup } from './StepStatusTree.svelte';

  interface Props {
    /** The steps status of the reply, as its Markdown table. */
    table: string;
    /** The message the table is in, and what the chat knows of the reply around it. */
    messageKey: string;
    items: TranscriptItem[];
    toolCalls: Record<string, ToolCallState>;
    memoryUse?: MemoryUseIndex;
    knowledgeUse?: KnowledgeUseIndex;
  }

  let { table, messageKey, items, toolCalls, memoryUse, knowledgeUse }: Props = $props();

  let open = $state(false);

  /** The rows the app knows: a label it can translate and an icon. Another row a hook added shows as the agent wrote it. */
  const KNOWN: Record<string, { label: MessageKey; icon: Component }> = {
    Limits: { label: 'chat.stepStatus.label.limits', icon: Gauge },
    Branch: { label: 'chat.stepStatus.label.branch', icon: GitBranch },
    Notes: { label: 'chat.stepStatus.label.notes', icon: NotebookText },
    Scripts: { label: 'chat.stepStatus.label.scripts', icon: FileCode },
    Knowledge: { label: 'chat.stepStatus.label.knowledge', icon: BookOpenText },
    'Code Graph': { label: 'chat.stepStatus.label.codeGraph', icon: Waypoints },
  };

  const LEVEL_LABELS: Record<StepLevel, MessageKey> = {
    easy: 'chat.stepStatus.level.easy',
    medium: 'chat.stepStatus.level.medium',
    hard: 'chat.stepStatus.level.hard',
  };
  const LEVEL_BARS: Record<StepLevel, number> = { easy: 1, medium: 2, hard: 3 };

  const KIND_LABELS: Record<CodeLookup, MessageKey> = {
    map: 'chat.stepStatus.kind.map',
    impact: 'chat.stepStatus.kind.impact',
    path: 'chat.stepStatus.kind.path',
    symbol: 'chat.stepStatus.kind.symbol',
  };

  const step = $derived(readStepStatus(table));

  // Read only while the card is open: a closed card costs a chat nothing on each delta.
  const reply = $derived(open ? replyOf(items, messageKey, toolCalls) : null);
  const notes = $derived(reply ? checkedNotes(reply, memoryUse) : []);
  const docs = $derived(reply ? checkedDocs(reply, knowledgeUse) : []);
  const code = $derived(reply ? readCodeGraph(reply.calls) : { lookups: 0, files: [] });

  const labelOf = (label: string): string => (KNOWN[label] ? t(KNOWN[label].label) : label);

  /** What the reply did with a row, as a badge. */
  function useBadges(use: RowUse | null): TreeBadge[] {
    if (use?.kind === 'useful') {
      return [{ label: t('chat.family.row.useful'), tone: 'info', icon: ThumbsUp }];
    }

    return use ? [{ label: t('chat.family.row.opened'), tone: 'neutral', icon: Eye }] : [];
  }

  function reasonOf(use: RowUse | null): string | null {
    return use?.kind === 'useful' && use.reason ? t('chat.family.row.reason', { reason: use.reason }) : null;
  }

  function checkedGroup<Row>(group: CheckedGroup<Row>, rowOf: (row: Row, use: RowUse | null) => TreeGroup['rows'][number]): TreeGroup {
    return {
      key: group.key,
      label: group.query ?? t(group.recalled ? 'chat.stepStatus.forMessage' : 'chat.stepStatus.searched'),
      mono: group.query !== null,
      rows: group.rows.map(({ row, use }) => rowOf(row, use)),
    };
  }

  const noteGroups = $derived(
    notes.map((group) =>
      checkedGroup(group, (note, use) => ({
        key: String(note.id),
        title: note.title,
        detail: reasonOf(use),
        quiet: use === null,
        badges: useBadges(use),
        onopen: () => showNote(note.id, note.scope),
      })),
    ),
  );

  const docGroups = $derived(
    docs.map((group) =>
      checkedGroup(group, (doc, use) => ({
        key: doc.id,
        title: doc.title,
        detail: [doc.section, reasonOf(use)].filter(Boolean).join(' · ') || null,
        quiet: use === null,
        badges: useBadges(use),
        onopen: () => showDocument(doc.id),
      })),
    ),
  );

  /** The files by folder, in the order the lookups named them. */
  const fileGroups = $derived.by<TreeGroup[]>(() => {
    const folders = new Map<string, TreeGroup>();

    for (const file of code.files) {
      const slash = file.path.lastIndexOf('/');
      const folder = slash > 0 ? file.path.slice(0, slash) : '.';
      const group = folders.get(folder) ?? { key: folder, label: folder, mono: true, rows: [] };

      group.rows.push({
        key: file.path,
        title: file.path.slice(slash + 1),
        mono: true,
        badges: file.kinds.map((kind) => ({ label: t(KIND_LABELS[kind]), tone: 'info' })),
      });
      folders.set(folder, group);
    }

    return [...folders.values()];
  });

  function checkedSummary(groups: CheckedGroup<unknown>[]): string {
    const { checked, used } = checkedCounts(groups);

    return used > 0 ? t('chat.stepStatus.used', { used, checked }) : t('chat.stepStatus.noneUsed', { count: checked });
  }

  const codeSummary = $derived(`${tn('chat.stepStatus.lookups', code.lookups)}, ${tn('chat.stepStatus.files', code.files.length)}`);

  /** A row that has real results shows them, whatever the agent wrote. Else one that says none joins the "No match" line. */
  function hasResults(label: string): boolean {
    return (label === 'Notes' && notes.length > 0) || (label === 'Knowledge' && docs.length > 0) || (label === 'Code Graph' && code.files.length > 0);
  }

  const noMatch = $derived(step.rows.filter((row) => saysNone(row.value) && !hasResults(row.label)).map((row) => labelOf(row.label)));
  const listed = $derived(step.rows.filter((row) => !saysNone(row.value) || hasResults(row.label)));
</script>

<ChatRow {open} ontoggle={() => (open = !open)} data-step-status="">
  {#snippet icon()}<ClipboardList size={16} aria-hidden="true" />{/snippet}

  {#snippet title()}
    <span class="font-medium text-fg">{t('chat.stepStatus.title')}</span>
  {/snippet}

  {#snippet status()}
    {#if step.level}
      {@const level = step.level}
      <StatusPill tone="info" label={t(LEVEL_LABELS[level])}>
        {#snippet icon()}
          <span class="flex items-center gap-0.5" aria-hidden="true">
            {#each [1, 2, 3] as bar (bar)}
              <i class={cx('h-3 w-1 rounded-full bg-current', bar > LEVEL_BARS[level] && 'opacity-25')}></i>
            {/each}
          </span>
        {/snippet}
      </StatusPill>
    {/if}
  {/snippet}

  {#snippet body()}
    <div class="flex flex-col gap-0.5 p-1.5">
      {#if step.reason}
        <StepStatusRow icon={Layers} label={t('chat.stepStatus.label.complexity')} value={step.reason} />
      {/if}

      {#each listed as row, index (`${row.label}:${index}`)}
        {@const icon = KNOWN[row.label]?.icon ?? CircleDot}

        {#if row.label === 'Notes' && notes.length > 0}
          <StepStatusRow {icon} label={labelOf(row.label)} summary={checkedSummary(notes)}>
            {#snippet tree()}<StepStatusTree groups={noteGroups} groupIcon={Search} rowIcon={NotebookText} />{/snippet}
          </StepStatusRow>
        {:else if row.label === 'Knowledge' && docs.length > 0}
          <StepStatusRow {icon} label={labelOf(row.label)} summary={checkedSummary(docs)}>
            {#snippet tree()}<StepStatusTree groups={docGroups} groupIcon={Search} rowIcon={BookOpenText} />{/snippet}
          </StepStatusRow>
        {:else if row.label === 'Code Graph' && code.files.length > 0}
          <StepStatusRow {icon} label={labelOf(row.label)} summary={codeSummary}>
            {#snippet tree()}<StepStatusTree groups={fileGroups} groupIcon={Folder} rowIcon={File} />{/snippet}
          </StepStatusRow>
        {:else}
          <StepStatusRow {icon} label={labelOf(row.label)} value={row.value} />
        {/if}
      {/each}

      {#if noMatch.length > 0}
        <StepStatusRow icon={SearchX} label={t('chat.stepStatus.noMatch')} value={noMatch.join(', ')} quiet />
      {/if}
    </div>
  {/snippet}
</ChatRow>

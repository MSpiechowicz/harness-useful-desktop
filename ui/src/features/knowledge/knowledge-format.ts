import { type FactRow, momentFact } from '$lib/components/facts';
import { shortRepoName } from '$lib/components/scope/scope-options';
import { COLUMN_WIDTH, type Column } from '$lib/components/table';
import { type MessageKey, t, tn } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';
import type { KnowledgeSort } from '$lib/ipc/bindings/KnowledgeSort';
import type { KnowledgeId } from '$lib/ipc/knowledge-id';
import { addedByFact, updatedByFact } from '$lib/utils/author-format';
import { formatBytes } from '$lib/utils/format';
import { KNOWLEDGE_FORMATS } from '$lib/utils/knowledge-formats';

/** The formats the Format filter lists, in the order of the menu. */
export { KNOWLEDGE_FORMATS };

const FORMAT_LABELS: Record<string, MessageKey> = {
  markdown: 'knowledge.format.markdown',
  text: 'knowledge.format.text',
  html: 'knowledge.format.html',
  openapi: 'knowledge.format.openapi',
  asciidoc: 'knowledge.format.asciidoc',
  rst: 'knowledge.format.rst',
  pdf: 'knowledge.format.pdf',
  transcript: 'knowledge.format.transcript',
};

type Tone = 'neutral' | 'accent' | 'success' | 'warning' | 'info';

const FORMAT_TONES: Record<string, Tone> = { markdown: 'accent', openapi: 'success', pdf: 'warning', asciidoc: 'info', html: 'neutral', rst: 'neutral', text: 'neutral', transcript: 'info' };

/** What the Format chip of a document shows: the format the server read it as (`transcript` for a `.vtt`/`.srt` file). */
export function formatKey(doc: Pick<KnowledgeDocument, 'format'>): string {
  return doc.format;
}

/** The name of a format for people; a format this app does not know reads as it is written, capitalised. */
export function formatLabel(format: string): string {
  const key = FORMAT_LABELS[format];

  return key ? t(key) : format.toUpperCase();
}

export function formatTone(format: string): Tone {
  return FORMAT_TONES[format] ?? 'neutral';
}

/** Where a document is kept, for people. The words are the ones of the Kept in settings. */
export function placeLabel(place: KnowledgePlace): string {
  return t(place === 'local' ? 'settings.memoryRemote.target.local' : 'settings.memoryRemote.target.remote');
}

/** A repository key for people: `acme/widget` for `git:github.com/acme/widget`, the folder of a `path:` key. */
export function repoLabel(key: string): string {
  return shortRepoName(key.replace(/^(git|path):/, ''));
}

/** How many paths of its repositories a document is narrowed to. */
export function pathCount(links: readonly KnowledgeLink[]): number {
  return links.reduce((sum, link) => sum + link.globs.length, 0);
}

/** The columns the server sorts the documents by. A key that is not one (a state left by an older version) sorts by time. */
const SORTS: Record<string, KnowledgeSort> = { title: 'title', format: 'format', repos: 'repos', chunks: 'chunks', updated: 'updated' };

/** The two columns of usefulness, which only a Harness Knowledge that counts it sorts by. */
export const USEFUL_SORTS: readonly string[] = ['useful', 'lastUseful'];

export function sortOf(key: string, usefulKnown = true): KnowledgeSort {
  if (usefulKnown && (key === 'useful' || key === 'lastUseful')) {
    return key;
  }

  return SORTS[key] ?? 'updated';
}

/**
 * The columns of the documents table. The two of usefulness exist only for a Harness Knowledge that counts it, and "Added by" only for one that tells who added a
 * document: an older one has nothing to show in them. Who added a document is not sortable, the server has no sort by it.
 */
export function docColumns(usefulness = false, authors = false): Column[] {
  const useful: Column[] = [
    { key: 'useful', label: t('knowledge.column.useful'), width: COLUMN_WIDTH.useful, from: 'xl', sortable: true, descFirst: true },
    { key: 'lastUseful', label: t('knowledge.column.lastUseful'), width: COLUMN_WIDTH.lastUseful, from: 'xl', sortable: true, descFirst: true },
  ];

  return [
    { key: 'title', label: t('knowledge.column.title'), sortable: true },
    { key: 'format', label: t('knowledge.column.format'), width: '8.5rem', sortable: true, from: 'sm' },
    { key: 'repos', label: t('knowledge.column.repos'), width: '12rem', sortable: true, from: 'md' },
    { key: 'chunks', label: t('knowledge.column.chunks'), width: '6rem', align: 'end', sortable: true, descFirst: true, from: 'lg' },
    ...(usefulness ? useful : []),
    ...(authors ? [{ key: 'addedBy', label: t('knowledge.column.addedBy'), width: COLUMN_WIDTH.author, from: 'xl' } satisfies Column] : []),
    { key: 'updated', label: t('knowledge.column.updated'), width: '8rem', sortable: true, descFirst: true, from: 'sm' },
  ];
}

export function heldColumns(): Column[] {
  return [
    { key: 'title', label: t('knowledge.column.proposal'), sortable: true },
    { key: 'client', label: t('knowledge.column.client'), width: '11rem', sortable: true, from: 'sm' },
    { key: 'project', label: t('knowledge.column.project'), width: '13rem', sortable: true, from: 'md' },
    { key: 'proposed', label: t('knowledge.column.proposed'), width: '10rem', sortable: true, descFirst: true, from: 'sm' },
  ];
}

/** The folder name of a project path, for the Project column. */
export function folderName(path: string | null): string {
  return path?.split(/[\\/]/).filter(Boolean).at(-1) ?? '';
}

/** The Details card of a document. `sections` is how many headings the outline lists, which the document alone does not say. */
export function docFacts(doc: KnowledgeDocument, sections: number | null): FactRow[] {
  return [
    { label: t('knowledge.fact.format'), value: formatLabel(formatKey(doc)) },
    ...(doc.place ? [{ label: t('settings.memoryRemote.section.keptIn'), value: placeLabel(doc.place) }] : []),
    { label: t('knowledge.fact.size'), value: formatBytes(doc.bytes) },
    { label: t('knowledge.fact.chunks'), value: String(doc.chunkCount) },
    ...(sections === null ? [] : [{ label: t('knowledge.fact.sections'), value: String(sections) }]),
    { label: t('knowledge.fact.source'), value: doc.sourceName, mono: true, title: doc.sourceKey },
    ...addedByFact(doc),
    momentFact(t('knowledge.fact.updated'), doc.updatedAt),
    ...updatedByFact(doc),
    // Who added it says more than how it came in, so Origin only stands in while the name is not known.
    ...(doc.addedBy ? [] : [{ label: t('knowledge.fact.origin'), value: t(doc.origin === 'agentApproved' ? 'knowledge.origin.agent' : 'knowledge.origin.user') }]),
  ];
}

/** The Details card of a proposal. */
export function heldFacts(item: KnowledgeHeldItem): FactRow[] {
  return [
    { label: t('knowledge.fact.format'), value: formatLabel(item.format) },
    { label: t('knowledge.fact.size'), value: formatBytes(item.bytes) },
    { label: t('knowledge.fact.source'), value: item.sourcePath ?? item.name, mono: true },
    { label: t('knowledge.fact.client'), value: item.client },
    ...(item.projectPath === null ? [] : [{ label: t('knowledge.fact.project'), value: folderName(item.projectPath), title: item.projectPath }]),
    momentFact(t('knowledge.fact.proposed'), item.createdAt),
    ...(item.replaces === null ? [] : [{ label: t('knowledge.fact.replaces'), value: t('knowledge.fact.replaces.value') }]),
  ];
}

/** The line under a title: the format, then the repositories (or Global). */
export function docSubtitle(doc: Pick<KnowledgeDocument, 'format' | 'sourceName' | 'links' | 'place'>): string {
  return [formatLabel(formatKey(doc)), ...(doc.place ? [placeLabel(doc.place)] : []), linksText(doc.links)].join(' · ');
}

/** Where documents apply as one line: `acme/billing, acme/web`, or Global. */
export function linksText(links: readonly KnowledgeLink[]): string {
  return links.length === 0 ? t('knowledge.repos.global') : links.map((link) => repoLabel(link.repo)).join(', ');
}

/** "3 paths", or nothing for a whole repository. */
export function pathsText(count: number): string | null {
  return count === 0 ? null : tn('knowledge.repos.paths', count);
}

/** The outline of a document: the server lists one entry per chunk, so the chunks a long section is cut into are one heading here. */
export function outlineOf<Section extends { headingPath: string }>(sections: readonly Section[]): Section[] {
  return sections.filter((section, index) => index === 0 || sections[index - 1]?.headingPath !== section.headingPath);
}

/** One heading of the outline with every chunk it is cut into, in document order. */
export interface OutlineGroup {
  headingPath: string;
  chunkIds: KnowledgeId[];
}

/** The outline with the chunk ids of each heading kept, so the text of a section is all its parts. */
export function outlineGroups(sections: readonly { headingPath: string; chunkId: KnowledgeId }[]): OutlineGroup[] {
  const groups: OutlineGroup[] = [];

  for (const section of sections) {
    const last = groups.at(-1);

    if (last?.headingPath === section.headingPath) {
      last.chunkIds.push(section.chunkId);
    } else {
      groups.push({ headingPath: section.headingPath, chunkIds: [section.chunkId] });
    }
  }

  return groups;
}

/** A heading path as the levels it has and its own title: `Guide > Install > Linux` is the third level, "Linux". */
export function headingLevel(headingPath: string): { depth: number; title: string } {
  const parts = headingPath.split('>').map((part) => part.trim()).filter(Boolean);

  return { depth: Math.max(0, parts.length - 1), title: parts.at(-1) ?? t('knowledge.sections.intro') };
}

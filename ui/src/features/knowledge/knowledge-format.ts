import { type FactRow, momentFact } from '$lib/components/facts';
import { shortRepoName } from '$lib/components/scope/scope-options';
import type { Column } from '$lib/components/table';
import { type MessageKey, t, tn } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
import type { KnowledgeSort } from '$lib/ipc/bindings/KnowledgeSort';
import { formatBytes } from '$lib/utils/format';

/** The formats the Format filter lists, in the order of the menu. */
export const KNOWLEDGE_FORMATS = ['markdown', 'text', 'html', 'openapi', 'asciidoc', 'rst', 'pdf', 'transcript'] as const;

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

/** A repository key for people: `acme/widget` for `git:github.com/acme/widget`, the folder of a `path:` key. */
export function repoLabel(key: string): string {
  return shortRepoName(key.replace(/^(git|path):/, ''));
}

/** How many paths of its repositories a document is narrowed to. */
export function pathCount(links: readonly KnowledgeLink[]): number {
  return links.reduce((sum, link) => sum + link.globs.length, 0);
}

/** The columns the server sorts the documents by: the title and the time. A key that is not one (a state left by an older version) sorts by time. */
const SORTS: Record<string, KnowledgeSort> = { title: 'title', updated: 'updated' };

export function sortOf(key: string): KnowledgeSort {
  return SORTS[key] ?? 'updated';
}

export function docColumns(): Column[] {
  return [
    { key: 'title', label: t('knowledge.column.title'), sortable: true },
    { key: 'format', label: t('knowledge.column.format'), width: '9.5rem', from: 'sm' },
    { key: 'repos', label: t('knowledge.column.repos'), width: '16rem', from: 'md' },
    { key: 'chunks', label: t('knowledge.column.chunks'), width: '6rem', align: 'end', from: 'lg' },
    { key: 'updated', label: t('knowledge.column.updated'), width: '8rem', sortable: true, descFirst: true, from: 'sm' },
  ];
}

export function heldColumns(): Column[] {
  return [
    { key: 'title', label: t('knowledge.column.proposal') },
    { key: 'client', label: t('knowledge.column.client'), width: '10rem', from: 'sm' },
    { key: 'project', label: t('knowledge.column.project'), width: '12rem', from: 'md' },
    { key: 'proposed', label: t('knowledge.column.proposed'), width: '8rem', from: 'sm' },
  ];
}

/** The folder name of a project path, for the Project column. */
export function folderName(path: string | null): string {
  return path?.split(/[\\/]/).filter(Boolean).at(-1) ?? '';
}

/** The Details card of a document. `sections` is how many chunks the outline lists, which the document alone does not say. */
export function docFacts(doc: KnowledgeDocument, sections: number | null): FactRow[] {
  return [
    { label: t('knowledge.fact.format'), value: formatLabel(formatKey(doc)) },
    { label: t('knowledge.fact.size'), value: formatBytes(doc.bytes) },
    { label: t('knowledge.fact.chunks'), value: String(doc.chunkCount) },
    ...(sections === null ? [] : [{ label: t('knowledge.fact.sections'), value: String(sections) }]),
    { label: t('knowledge.fact.source'), value: doc.sourceName, mono: true, title: doc.sourceKey },
    momentFact(t('knowledge.fact.updated'), doc.updatedAt),
    { label: t('knowledge.fact.origin'), value: t(doc.origin === 'agentApproved' ? 'knowledge.origin.agent' : 'knowledge.origin.user') },
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
export function docSubtitle(doc: Pick<KnowledgeDocument, 'format' | 'sourceName' | 'links'>): string {
  return [formatLabel(formatKey(doc)), linksText(doc.links)].join(' · ');
}

/** Where documents apply as one line: `acme/billing, acme/web`, or Global. */
export function linksText(links: readonly KnowledgeLink[]): string {
  return links.length === 0 ? t('knowledge.repos.global') : links.map((link) => repoLabel(link.repo)).join(', ');
}

/** "3 paths", or nothing for a whole repository. */
export function pathsText(count: number): string | null {
  return count === 0 ? null : tn('knowledge.repos.paths', count);
}

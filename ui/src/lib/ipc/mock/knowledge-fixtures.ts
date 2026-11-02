import type { AuthorRef } from '../bindings/AuthorRef';
import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeHeldItem } from '../bindings/KnowledgeHeldItem';
import type { KnowledgeId } from '../bindings/KnowledgeId';
import type { KnowledgeLink } from '../bindings/KnowledgeLink';
import type { KnowledgeOrigin } from '../bindings/KnowledgeOrigin';
import type { KnowledgeSection } from '../bindings/KnowledgeSection';
import { ADA, GRACE, GRACE_AGENT } from './authors';
import { fakeSha } from './fake-sha';
import { numberOf } from './knowledge-routing';

const HOUR = 3_600_000;

/** The repositories the sample documents link to. */
export const DESKTOP_REPO = 'git:github.com/useful/harness-desktop';
export const WEBSITE_REPO = 'git:github.com/useful/website';

function ago(hours: number): string {
  return new Date(Date.now() - hours * HOUR).toISOString();
}

/** How often agents were shown a document and found it useful (the last time, hours ago). */
export interface Usage {
  shown: number;
  useful: number;
  lastUseful?: number;
}

export interface Seed {
  id: KnowledgeId;
  title: string;
  name: string;
  format: string;
  kb: number;
  chunks: number;
  age: number;
  links?: KnowledgeLink[];
  origin?: KnowledgeOrigin;
  warnings?: string[];
  /** Who added and last changed it. Left out: nobody (no git name was known). */
  addedBy?: AuthorRef;
  updatedBy?: AuthorRef;
  usage?: Usage;
}

export function documentOf(seed: Seed): KnowledgeDocument {
  const { usage } = seed;

  return {
    id: seed.id,
    title: seed.title,
    sourceName: seed.name,
    sourceKey: `docs/${seed.name}`,
    format: seed.format,
    contentSha256: fakeSha([seed.id, 'text']),
    fileSha256: fakeSha([seed.id, 'file']),
    bytes: seed.kb * 1024,
    chunkCount: seed.chunks,
    createdAt: ago(seed.age * 2),
    updatedAt: ago(seed.age),
    origin: seed.origin ?? 'user',
    warnings: seed.warnings ?? [],
    links: seed.links ?? [],
    addedBy: seed.addedBy ?? null,
    updatedBy: seed.updatedBy ?? seed.addedBy ?? null,
    shown: usage?.shown ?? 0,
    useful: usage?.useful ?? 0,
    lastUsefulAt: usage?.lastUseful === undefined ? null : ago(usage.lastUseful),
  };
}

/** `doc` as a server without `docs-useful` and `docs-authors` sends it: no authors and no counts. */
export function withoutQuality(doc: KnowledgeDocument): KnowledgeDocument {
  const { addedBy: _added, updatedBy: _updated, shown: _shown, useful: _useful, lastUsefulAt: _last, ...rest } = doc;

  return rest;
}

const DESKTOP: KnowledgeLink[] = [{ repo: DESKTOP_REPO, globs: [] }];

/** The documents of the preview: every format, a meeting transcript, global ones and ones linked to a repository or to paths in it. */
export function sampleDocuments(): KnowledgeDocument[] {
  return [
    documentOf({ id: 1, title: 'Architecture overview', name: 'architecture.md', format: 'markdown', kb: 42, chunks: 18, age: 3, links: DESKTOP, addedBy: ADA, updatedBy: GRACE, usage: { shown: 24, useful: 11, lastUseful: 5 } }),
    documentOf({ id: 2, title: 'Billing API', name: 'billing-openapi.yaml', format: 'openapi', kb: 188, chunks: 64, age: 20, links: [{ repo: DESKTOP_REPO, globs: ['src/billing/**', 'src/invoices/**'] }], addedBy: GRACE, usage: { shown: 9, useful: 4, lastUseful: 30 } }),
    documentOf({ id: 3, title: 'Standup 28 October (transcript)', name: 'standup-2026-10-28.vtt', format: 'transcript', kb: 31, chunks: 9, age: 30, warnings: ['No speaker names were found, so the lines are not attributed.'], addedBy: ADA, usage: { shown: 7, useful: 0 } }),
    documentOf({ id: 4, title: 'Security review checklist', name: 'security-review.pdf', format: 'pdf', kb: 920, chunks: 77, age: 52, usage: { shown: 3, useful: 0 } }),
    documentOf({ id: 5, title: 'Release process', name: 'release-process.adoc', format: 'asciidoc', kb: 27, chunks: 12, age: 75, links: [{ repo: WEBSITE_REPO, globs: [] }, ...DESKTOP], addedBy: GRACE, usage: { shown: 14, useful: 6, lastUseful: 80 } }),
    documentOf({ id: 6, title: 'Onboarding for new engineers', name: 'onboarding.html', format: 'html', kb: 64, chunks: 21, age: 96, addedBy: ADA, usage: { shown: 2, useful: 0 } }),
    documentOf({ id: 7, title: 'Error codes', name: 'errors.rst', format: 'rst', kb: 18, chunks: 7, age: 130, links: DESKTOP, addedBy: ADA, updatedBy: GRACE, usage: { shown: 5, useful: 1, lastUseful: 140 } }),
    documentOf({ id: 8, title: 'Incident 2026-09 postmortem', name: 'postmortem-2026-09.md', format: 'markdown', kb: 12, chunks: 5, age: 200, origin: 'agentApproved', links: DESKTOP, addedBy: GRACE_AGENT, usage: { shown: 4, useful: 3, lastUseful: 210 } }),
    documentOf({ id: 9, title: 'Design tokens', name: 'tokens.md', format: 'markdown', kb: 9, chunks: 4, age: 260, links: [{ repo: DESKTOP_REPO, globs: ['ui/src/lib/design/**'] }] }),
    documentOf({ id: 10, title: 'Glossary', name: 'glossary.txt', format: 'text', kb: 6, chunks: 3, age: 300 }),
  ];
}

/** `count` more documents for the scale of a long list, ids after the sample ones. */
export function bulkDocuments(count: number): KnowledgeDocument[] {
  const formats = ['markdown', 'text', 'html', 'pdf'];

  return Array.from({ length: count }, (_, index) => {
    const id = 11 + index;

    return documentOf({ id, title: `Runbook ${id}`, name: `runbook-${id}.md`, format: formats[index % formats.length], kb: 4 + (index % 40), chunks: 2 + (index % 15), age: 320 + index, addedBy: index % 4 === 0 ? undefined : index % 2 === 0 ? ADA : GRACE, usage: index % 5 === 0 ? undefined : { shown: index % 11, useful: index % 3 === 0 ? 0 : index % 4, lastUseful: index % 3 === 0 ? undefined : 400 + (index % 90) } });
  });
}

/** The outline of a document: a few sections per document, derived from its chunk count. A longer document cuts Configuration into two chunks, as the server does a long section. */
export function outlineOf(doc: KnowledgeDocument): KnowledgeSection[] {
  const names = ['Overview', 'Install', 'Configuration', 'Reference', 'Troubleshooting'];
  const headings = doc.chunkCount >= 6 ? [...names.slice(0, 3), 'Configuration', ...names.slice(3)] : names.slice(0, Math.max(1, doc.chunkCount));

  return headings.map((headingPath, ordinal) => ({
    chunkId: doc.place === 'local' ? `local:${numberOf(doc.id) * 100 + ordinal}` : numberOf(doc.id) * 100 + ordinal,
    ordinal,
    headingPath,
    tokens: 220 + ordinal * 40,
    charStart: ordinal * 900,
    charEnd: ordinal * 900 + 880,
  }));
}

/** The text of one chunk of a document: a few paragraphs, with a code block under Install. */
export function chunkText(doc: KnowledgeDocument, section: KnowledgeSection): string {
  const lines = [
    `This part of ${doc.title} describes the ${section.headingPath.toLowerCase()} in detail. Upgrading from version 1 to version 2 keeps your settings and your history.`,
    '',
    'Read the steps in order. A step that fails leaves everything as it was, so it is safe to run it again once the cause is fixed.',
  ];

  if (section.headingPath === 'Install') {
    lines.push('', '```sh', 'harness install --version 1', 'harness doctor', '```');
  }

  return lines.join('\n');
}

function held(seed: { id: string; title: string; name: string; text: string; client: string; age: number; links?: KnowledgeLink[]; replaces?: number }): KnowledgeHeldItem {
  return {
    id: seed.id,
    title: seed.title,
    format: 'markdown',
    name: seed.name,
    bytes: seed.text.length,
    text: seed.text,
    links: seed.links ?? DESKTOP,
    client: seed.client,
    session: 'a1b2c3',
    projectPath: '/home/dev/projects/harness-desktop',
    repoKey: DESKTOP_REPO,
    sourcePath: `docs/${seed.name}`,
    replaces: seed.replaces ?? null,
    revision: fakeSha(seed.id).slice(0, 16),
    createdAt: ago(seed.age),
  };
}

/** The proposals waiting for review in the preview. */
export function sampleHeld(): KnowledgeHeldItem[] {
  return [
    held({ id: '0b6f2f0e-7f63-4c0e-9d1a-0d5a4a0a7e11', title: 'Install notes for Windows', name: 'install-windows.md', client: 'claude-code', age: 1, text: '# Install notes for Windows\n\nRun the installer from PowerShell and restart the shell so the new PATH applies.' }),
    held({ id: '7a1c3d52-1d8e-4b57-8f9b-2c7e5a1f9b22', title: 'Billing API', name: 'billing-openapi.yaml', client: 'codex', age: 6, replaces: 2, text: '# Billing API\n\nThe retry rules changed: a failed charge is retried three times, one hour apart.' }),
    held({ id: '3e9d8a71-5c20-4a6d-b1f3-9f4e6d2c8a33', title: 'Local database reset', name: 'db-reset.md', client: 'opencode', age: 27, links: [], text: '# Local database reset\n\nDelete the data folder and start the app again to get an empty database.' }),
  ];
}

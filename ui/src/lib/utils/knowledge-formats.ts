/** The formats the Knowledge view knows, in the order of its Format menu. */
export const KNOWLEDGE_FORMATS = ['markdown', 'text', 'html', 'openapi', 'asciidoc', 'rst', 'pdf', 'transcript'] as const;

/** `format` when it is one the Knowledge view knows, else null: an agent-facing reply is text, so a format in it is not trusted to be one. */
export function knownFormat(format: string | undefined): string | null {
  return (KNOWLEDGE_FORMATS as readonly string[]).includes(format ?? '') ? (format ?? null) : null;
}

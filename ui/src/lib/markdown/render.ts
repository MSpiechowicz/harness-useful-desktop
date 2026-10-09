import MarkdownIt, { type Token } from 'markdown-it';

import { sanitize } from './sanitize';

/** A run of rendered (and sanitized) Markdown, or a fenced code block shown by the code component. */
export type Segment = { type: 'html'; html: string } | { type: 'code'; lang: string; code: string };

/** Attribute values can be numbers in the typings; hrefs are always strings. */
function hrefOf(token: Token): string {
  return String(token.attrGet('href') ?? '');
}

function isWebLink(href: string): boolean {
  try {
    const { protocol } = new URL(href);

    return protocol === 'https:' || protocol === 'http:';
  } catch {
    return false;
  }
}

function createParser() {
  // `html: false` makes raw HTML in the input plain text; links and images are re-rendered below.
  const md = new MarkdownIt({ html: false, linkify: true, typographer: false, breaks: false });

  // Links never navigate: they become buttons that carry the target, and only web links keep one.
  md.renderer.rules.link_open = (tokens, index) => {
    const href = hrefOf(tokens[index]);
    if (!isWebLink(href)) {
      return '<span class="md-link-text">';
    }

    const escaped = md.utils.escapeHtml(href);

    return `<button type="button" class="md-link" data-href="${escaped}" title="${escaped}">`;
  };

  md.renderer.rules.link_close = (tokens, index) => (closesWebLink(tokens, index) ? '</button>' : '</span>');

  // Remote images would load on render; show what they say instead.
  md.renderer.rules.image = (tokens, index) => `<span class="md-image">[image: ${md.utils.escapeHtml(tokens[index].content || 'untitled')}]</span>`;

  // Nested fences (inside lists) keep a plain block; top-level ones are split out by `toSegments`.
  md.renderer.rules.fence = (tokens, index) => `<pre class="md-pre"><code>${md.utils.escapeHtml(tokens[index].content)}</code></pre>`;

  return md;
}

/** Walks back to the opening token of this link to learn whether it was a web link. */
function closesWebLink(tokens: Token[], closeIndex: number): boolean {
  for (let index = closeIndex - 1; index >= 0; index -= 1) {
    if (tokens[index].type === 'link_open') {
      return isWebLink(hrefOf(tokens[index]));
    }
  }

  return false;
}

const parser = createParser();

function langOf(info: string): string {
  return info.trim().split(/\s+/)[0]?.toLowerCase() ?? '';
}

/** Renders a token run to sanitized HTML. */
function htmlOf(tokens: Token[]): Segment[] {
  if (tokens.length === 0) {
    return [];
  }

  return [{ type: 'html', html: sanitize(parser.renderer.render(tokens, parser.options, {})) }];
}

/** Splits Markdown into sanitized HTML runs and top-level code fences (rendered by `CodeBlock`). */
export function toSegments(source: string): Segment[] {
  const segments: Segment[] = [];
  let run: Token[] = [];

  for (const token of parser.parse(source, {})) {
    if (token.type === 'fence' && token.level === 0) {
      segments.push(...htmlOf(run));
      run = [];
      segments.push({ type: 'code', lang: langOf(token.info), code: token.content.replace(/\n$/, '') });
    } else {
      run.push(token);
    }
  }

  return [...segments, ...htmlOf(run)];
}

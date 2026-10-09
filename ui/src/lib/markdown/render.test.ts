// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';

import { toSegments } from './render';
import { sanitize } from './sanitize';

function html(source: string): string {
  return toSegments(source)
    .flatMap((segment) => (segment.type === 'html' ? [segment.html] : []))
    .join('');
}

function dom(markup: string): HTMLElement {
  const root = document.createElement('div');
  root.append(...Array.from(new DOMParser().parseFromString(markup, 'text/html').body.childNodes));

  return root;
}

describe('toSegments', () => {
  it('renders raw HTML in the input as text, never as elements', () => {
    const root = dom(html('<img src=x onerror=alert(1)> <script>alert(1)</script> <b>bold?</b>'));

    expect(root.querySelector('img, script, b')).toBeNull();
    expect(root.textContent).toContain('<script>alert(1)</script>');
  });

  it('turns web links into inert buttons that carry their target', () => {
    const root = dom(html('See [the docs](https://example.com/docs) and https://example.org/x'));
    const buttons = Array.from(root.querySelectorAll('button[data-href]'));

    expect(root.querySelector('a')).toBeNull();
    expect(buttons.map((button) => button.getAttribute('data-href'))).toEqual(['https://example.com/docs', 'https://example.org/x']);
  });

  it('keeps javascript:, data: and relative links as plain text', () => {
    const root = dom(html('[a](javascript:alert(1)) [b](data:text/html,x) [c](/relative) [d](file:///etc/passwd)'));

    expect(root.querySelector('button, a')).toBeNull();
    expect(root.textContent).toContain('javascript:alert(1)');
  });

  it('never loads images; it shows what they say instead', () => {
    const root = dom(html('![diagram](https://example.com/p.png)'));

    expect(root.querySelector('img')).toBeNull();
    expect(root.textContent).toContain('[image: diagram]');
  });

  it('splits top-level code fences out and keeps their language', () => {
    const segments = toSegments('Intro\n\n```ts\nconst a = 1;\n```\n\nOutro');

    expect(segments.map((segment) => segment.type)).toEqual(['html', 'code', 'html']);
    expect(segments[1]).toEqual({ type: 'code', lang: 'ts', code: 'const a = 1;' });
  });

  it('shows an unfinished fence (mid-stream) as code', () => {
    const [segment] = toSegments('```rust\nfn main() {');

    expect(segment).toEqual({ type: 'code', lang: 'rust', code: 'fn main() {' });
  });

  it('keeps fences nested in lists as plain preformatted text', () => {
    const root = dom(html('1. Run:\n\n   ```bash\n   cargo test\n   ```'));

    expect(root.querySelector('pre code')?.textContent).toContain('cargo test');
  });
});

describe('sanitize', () => {
  it('drops everything outside the allowlist', () => {
    const clean = sanitize('<p onclick="x()">ok</p><script>bad()</script><iframe src="https://e.com"></iframe><img src="x" onerror="y()"><style>p{}</style>');

    expect(clean).toBe('<p>ok</p>');
  });

  it('removes a link target that is not a web address', () => {
    const root = dom(sanitize('<button class="md-link" data-href="javascript:alert(1)">x</button><button class="md-link" data-href="https://ok.example">y</button>'));
    const [bad, good] = Array.from(root.querySelectorAll('button'));

    expect(bad.hasAttribute('data-href')).toBe(false);
    expect(good.getAttribute('data-href')).toBe('https://ok.example');
  });
});

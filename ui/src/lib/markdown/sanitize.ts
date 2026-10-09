import DOMPurify from 'dompurify';

/** Only what the Markdown renderer emits; anything else (scripts, styles, forms, images, iframes) is dropped. */
const ALLOWED_TAGS = [
  'p', 'br', 'hr', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'strong', 'em', 'del', 's', 'code', 'pre', 'blockquote',
  'ul', 'ol', 'li', 'table', 'thead', 'tbody', 'tr', 'th', 'td', 'span', 'button',
];

const ALLOWED_ATTRIBUTES = ['class', 'title', 'type', 'data-href', 'start'];

/** Links are inert buttons that carry their target; only web links may keep it. */
const WEB_URL = /^https?:\/\//i;

let configured = false;

function configure(): void {
  if (configured) {
    return;
  }

  configured = true;
  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    const target = node.getAttribute?.('data-href');
    if (target != null && !WEB_URL.test(target)) {
      node.removeAttribute('data-href');
    }
  });
}

const CACHE_LIMIT = 200;
const cache = new Map<string, string>();

/** Sanitizes rendered Markdown HTML (memoized: streaming re-renders mostly repeat the same blocks). */
export function sanitize(html: string): string {
  const hit = cache.get(html);
  if (hit !== undefined) {
    return hit;
  }

  configure();
  const clean = DOMPurify.sanitize(html, {
    ALLOWED_TAGS,
    ALLOWED_ATTR: ALLOWED_ATTRIBUTES,
    ALLOW_DATA_ATTR: false,
    RETURN_TRUSTED_TYPE: false,
  });

  if (cache.size >= CACHE_LIMIT) {
    cache.delete(cache.keys().next().value as string);
  }
  cache.set(html, clean);

  return clean;
}

/** The longest branch name git accepts as a ref component path, in bytes (the backend refuses anything longer). */
const MAX_NAME_BYTES = 255;

/** Characters git forbids anywhere in a ref name, beside control characters and spaces. */
const FORBIDDEN = /[~^:?*[\\]/;

/**
 * Turns what the user typed into a branch name git would take: spaces become dashes, characters git forbids are
 * dropped, and the rest is tidied (no doubled separators, no leading dash, no component that starts with a dot or
 * ends in `.lock`). An empty result means nothing usable was typed.
 */
export function slugBranchName(input: string): string {
  const cleaned = input
    .trim()
    .replace(/\s+/g, '-')
    .replace(/[^\p{L}\p{N}._/-]/gu, '')
    .replace(/\.{2,}/g, '.')
    .replace(/-{2,}/g, '-');

  const parts = cleaned
    .split('/')
    .map((part) =>
      part
        .replace(/^\.+/, '')
        .replace(/\.+$/, '')
        .replace(/\.lock$/, ''),
    )
    .filter((part) => part !== '');

  return parts.join('/').replace(/^-+/, '');
}

/** Whether git accepts `name` as a new branch name, by the same rules the backend checks before it runs git. */
export function isBranchName(name: string): boolean {
  if (name === '' || name === '@' || name === 'HEAD' || name.startsWith('-')) {
    return false;
  }

  if (new TextEncoder().encode(name).length > MAX_NAME_BYTES) {
    return false;
  }

  if (/[\u0000-\u001f\u007f\s]/.test(name) || FORBIDDEN.test(name)) {
    return false;
  }

  if (name.includes('..') || name.includes('@{') || name.includes('//')) {
    return false;
  }

  if (name.startsWith('/') || name.endsWith('/') || name.endsWith('.')) {
    return false;
  }

  return name.split('/').every((part) => !part.startsWith('.') && !part.endsWith('.lock'));
}

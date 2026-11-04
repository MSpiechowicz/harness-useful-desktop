import { t } from '$lib/i18n/i18n.svelte';

/** The start of the key of a folder without a git remote: `path:/work/widget`. Such a key never leaves this computer, so a team document cannot link to it. */
const FOLDER_KEY = 'path:';

/** Whether a repository key names a folder without a git remote. */
export function isFolderKey(key: string): boolean {
  return key.startsWith(FOLDER_KEY);
}

/** The last folder of a `path:` key, the only part of it the app shows. A key with no folder in it reads as it is, without its prefix. */
export function folderOfKey(key: string): string {
  const path = key.startsWith(FOLDER_KEY) ? key.slice(FOLDER_KEY.length) : key;

  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/** The folders among `links` that have no git remote, by their last folder. */
export function folderNames(links: readonly { repo: string }[]): string[] {
  return [...new Set(links.filter((link) => isFolderKey(link.repo)).map((link) => folderOfKey(link.repo)))];
}

/** The toast of a team document the server would not link to a folder without a git remote: it names the folder when it is the one, and never shows a path. */
export function localPathToast(links: readonly { repo: string }[]): { title: string; description: string } {
  const folders = folderNames(links);

  return {
    title: t('knowledge.drafts.refusal.localPath.title'),
    description: folders.length === 1 ? t('knowledge.drafts.refusal.localPath.folder', { folder: folders[0] }) : t('knowledge.drafts.refusal.localPath.body'),
  };
}

/** What may stand before a path in a sentence: the start, a space, a quote, an opening bracket or a separator such as the colon of `path:`. */
const BEFORE_PATH = '(?<![^\\s"\'`\u2018\u2019\u201C\u201D(\\[{<,;:=])';

/** What may stand after a path: the end, a space, a quote, a closing bracket, punctuation, or a full stop that ends the sentence. A `/` or a letter means the path goes on. */
const AFTER_PATH = '(?=$|[\\s"\'`\u2018\u2019\u201C\u201D)\\]}>,;:!?]|\\.(?:\\s|$))';

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * `text` with the whole path of every `path:` link in it cut to its last folder: a sentence of the server must not show where on this computer a folder is.
 * Only whole paths are cut, the longest first, so a path that is the start of another one (or of a longer name) leaves both whole and rewrites nothing else.
 */
export function withoutFolderPaths(text: string, links: readonly { repo: string }[]): string {
  const folders = new Map<string, string>();

  for (const link of links) {
    const path = isFolderKey(link.repo) ? link.repo.slice(FOLDER_KEY.length) : '';
    const folder = folderOfKey(link.repo);

    if (path !== '' && path !== folder) {
      folders.set(path, folder);
    }
  }

  if (folders.size === 0) {
    return text;
  }

  const longestFirst = [...folders.keys()].sort((a, b) => b.length - a.length).map(escapeRegExp);
  // A whole path, or the folder part of a longer one (a file inside it, a trailing separator): only the folder's own path is replaced.
  const whole = new RegExp(`${BEFORE_PATH}(?:${longestFirst.join('|')})(?:${AFTER_PATH}|(?=[\\\\/]))`, 'g');

  return text.replace(whole, (path) => folders.get(path) ?? path);
}

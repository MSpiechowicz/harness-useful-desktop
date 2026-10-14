/** Folders kept in front of the file name when a path is long. */
const KEPT_FOLDERS = 2;

/**
 * A path for a narrow card: the home folder is `~`, and a long path keeps only its last folders behind `…/`, so
 * `/home/me/.omp/agent/mcp.json` reads `~/.omp/agent/mcp.json` and a deep one `…/profiles/work/mcp.json`.
 * Either separator is understood; the result uses `/`. The full path belongs in a tooltip.
 */
export function shortPath(path: string): string {
  const home = /^(?:\/home\/[^/\\]+|\/Users\/[^/\\]+|[A-Za-z]:[\\/]Users[\\/][^/\\]+)(?=[\\/]|$)/.exec(path);
  const rest = (home ? path.slice(home[0].length) : path).split(/[\\/]/).filter(Boolean);
  const keep = KEPT_FOLDERS + 1;

  if (rest.length <= keep) {
    return home ? `~/${rest.join('/')}` : path.startsWith('/') ? `/${rest.join('/')}` : rest.join('/');
  }

  return `…/${rest.slice(-keep).join('/')}`;
}

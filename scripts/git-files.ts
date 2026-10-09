// Shared helper for the guard scripts: enumerate authored files via git so that
// ignored directories (node_modules, target, dist, ...) are skipped automatically.
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

export const repoRoot = resolve(import.meta.dir, '..');

/** Tracked plus untracked-but-not-ignored files, as repo-relative POSIX paths. */
export function listFiles(): string[] {
  const result = Bun.spawnSync(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], {
    cwd: repoRoot,
  });
  if (!result.success) {
    throw new Error(`git ls-files failed: ${result.stderr.toString()}`);
  }

  return result.stdout
    .toString()
    .split('\0')
    .filter((path) => path !== '' && existsSync(join(repoRoot, path)));
}

export function readText(path: string): string {
  return readFileSync(join(repoRoot, path), 'utf8');
}

export function extensionOf(path: string): string {
  const dot = path.lastIndexOf('.');
  return dot === -1 ? '' : path.slice(dot);
}

/** True when `path` matches any of the glob patterns (Bun.Glob syntax, `**` crosses directories). */
export function matchesAny(path: string, patterns: readonly string[]): boolean {
  return patterns.some((pattern) => new Bun.Glob(pattern).match(path));
}

/** Print a titled violation list and exit non-zero. */
export function failWith(title: string, lines: string[]): never {
  console.error(`${title}\n${lines.map((line) => `  ${line}`).join('\n')}`);
  process.exit(1);
}

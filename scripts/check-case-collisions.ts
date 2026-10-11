// Guard: reject module paths that differ only in letter case.
//
// macOS and Windows file systems ignore case, so `./intro.svelte` (meaning intro.svelte.ts) resolves to
// Intro.svelte there and the build fails, while Linux builds it fine. Extensions an import may omit
// (.ts, .js) are dropped before comparing.
import { failWith, listFiles } from './git-files';

const byKey = new Map<string, string[]>();

for (const path of listFiles()) {
  const key = path.replace(/\.(ts|js)$/, '').toLowerCase();
  byKey.set(key, [...(byKey.get(key) ?? []), path]);
}

const collisions = [...byKey.values()].filter((paths) => paths.length > 1).map((paths) => paths.join('  <->  '));

if (collisions.length > 0) {
  failWith(`check-case-collisions: ${collisions.length} path(s) collide on case-insensitive file systems`, collisions);
}

console.log('check-case-collisions: ok');

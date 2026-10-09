// Linux dev helper: register a desktop entry for the dev build so its window shows the app icon.
//
//   bun scripts/linux-desktop-entry.ts            # install (or refresh)
//   bun scripts/linux-desktop-entry.ts --remove   # uninstall
//
// On Wayland the compositor ignores the icon a window sets on itself: it shows the icon of the desktop entry
// that matches the window's app ID. Harness Desktop's app ID is its binary name, `harness-desktop`. Installed
// deb/rpm packages ship that entry; a dev build (`bun run dev`, `target/debug/harness-desktop`) does not, so
// KDE and GNOME fall back to a generic icon. This writes only under $XDG_DATA_HOME (default ~/.local/share),
// never needs root, and `--remove` deletes exactly what it wrote. Remove it before installing a release
// package: a user entry with the same name hides the system one.
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const APP_ID = 'harness-desktop';
const ROOT = resolve(import.meta.dir, '..');
const ICONS = join(ROOT, 'src-tauri', 'icons');
const DATA_HOME = process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share');

/** The icons the bundler ships, by hicolor directory. Small sizes use the tighter crop (`logo-small.svg`). */
const ICON_FILES: { dir: string; source: string; name: string }[] = [
  { dir: '32x32/apps', source: '32x32.png', name: `${APP_ID}.png` },
  { dir: '128x128/apps', source: '128x128.png', name: `${APP_ID}.png` },
  { dir: '256x256/apps', source: '128x128@2x.png', name: `${APP_ID}.png` },
  { dir: '512x512/apps', source: 'icon.png', name: `${APP_ID}.png` },
  { dir: 'scalable/apps', source: 'source/logo-small.svg', name: `${APP_ID}.svg` },
];

const entryPath = join(DATA_HOME, 'applications', `${APP_ID}.desktop`);
const iconPath = (dir: string, name: string) => join(DATA_HOME, 'icons', 'hicolor', dir, name);

/** Quotes an Exec argument as the Desktop Entry spec requires when it contains spaces or reserved characters. */
function execArgument(value: string): string {
  if (!/[\s"'\\`$]/.test(value)) {
    return value;
  }

  return `"${value.replace(/(["`$\\])/g, '\\$1')}"`;
}

function desktopEntry(binary: string): string {
  return [
    '[Desktop Entry]',
    'Type=Application',
    'Name=Harness Desktop (dev)',
    'Comment=Development build of Harness Desktop',
    `Exec=${execArgument(binary)}`,
    `Icon=${APP_ID}`,
    `StartupWMClass=${APP_ID}`,
    'Terminal=false',
    'Categories=Development;',
    '',
  ].join('\n');
}

/** Asks the desktop to re-read entries and icons; a missing tool is fine (the next login picks the files up). */
function refreshCaches(): void {
  const tools: [string, string[]][] = [
    ['update-desktop-database', [dirname(entryPath)]],
    ['kbuildsycoca6', []],
  ];

  for (const [tool, args] of tools) {
    spawnSync(tool, args, { stdio: 'ignore' });
  }
}

function install(): void {
  const binary = join(ROOT, 'target', 'debug', APP_ID);

  mkdirSync(dirname(entryPath), { recursive: true });
  writeFileSync(entryPath, desktopEntry(binary));

  for (const icon of ICON_FILES) {
    const target = iconPath(icon.dir, icon.name);

    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(join(ICONS, icon.source), target);
  }

  refreshCaches();

  console.log(`Installed ${entryPath} and ${ICON_FILES.length} icons under ${join(DATA_HOME, 'icons', 'hicolor')}.`);
  console.log('Restart Harness Desktop to see the icon. Undo with: bun run desktop-entry -- --remove');
}

function remove(): void {
  const written = [entryPath, ...ICON_FILES.map((icon) => iconPath(icon.dir, icon.name))];

  for (const path of written) {
    if (existsSync(path)) {
      rmSync(path);
    }
  }

  refreshCaches();

  console.log('Removed the Harness Desktop (dev) desktop entry and its icons.');
}

if (process.platform !== 'linux') {
  console.log('Only needed on Linux: macOS and Windows take the icon from the app itself.');
} else if (process.argv.includes('--remove')) {
  remove();
} else {
  install();
}

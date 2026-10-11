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
//
// The icon's name carries a hash of the icon files (`harness-desktop-1a2b3c4d`): KWin and Plasma keep icons
// they loaded in memory by name for the whole session, so a new logo under the same name would not show
// until the next login. A changed logo gets a new name, and the previous icon files are removed.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const APP_ID = 'harness-desktop';
const ROOT = resolve(import.meta.dir, '..');
const ICONS = join(ROOT, 'src-tauri', 'icons');
const DATA_HOME = process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share');

/** The icons the bundler ships, by hicolor directory. Small sizes use the tighter crop (`logo-small.svg`). */
const ICON_FILES: { dir: string; source: string; extension: 'png' | 'svg' }[] = [
  { dir: '32x32/apps', source: '32x32.png', extension: 'png' },
  { dir: '128x128/apps', source: '128x128.png', extension: 'png' },
  { dir: '256x256/apps', source: '128x128@2x.png', extension: 'png' },
  { dir: '512x512/apps', source: 'icon.png', extension: 'png' },
  { dir: 'scalable/apps', source: 'source/logo-small.svg', extension: 'svg' },
];

/** Every icon file name this script has ever written: the plain name and any hashed one. */
const OWN_ICON = new RegExp(`^${APP_ID}(-[0-9a-f]{8})?\\.(png|svg)$`);

/** `harness-desktop-<first 8 hex of a SHA-256 over the icon files>`. */
function iconName(): string {
  const hash = createHash('sha256');

  for (const icon of ICON_FILES) {
    hash.update(readFileSync(join(ICONS, icon.source)));
  }

  return `${APP_ID}-${hash.digest('hex').slice(0, 8)}`;
}

const entryPath = join(DATA_HOME, 'applications', `${APP_ID}.desktop`);
const iconPath = (dir: string, name: string) => join(DATA_HOME, 'icons', 'hicolor', dir, name);

/** Quotes an Exec argument as the Desktop Entry spec requires when it contains spaces or reserved characters. */
function execArgument(value: string): string {
  if (!/[\s"'\\`$]/.test(value)) {
    return value;
  }

  return `"${value.replace(/(["`$\\])/g, '\\$1')}"`;
}

function desktopEntry(binary: string, icon: string): string {
  return [
    '[Desktop Entry]',
    'Type=Application',
    'Name=Harness Desktop (dev)',
    'Comment=Development build of Harness Desktop',
    `Exec=${execArgument(binary)}`,
    `Icon=${icon}`,
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

/** Deletes the icon files this script wrote, under any name it used. */
function removeIcons(): void {
  for (const icon of ICON_FILES) {
    const dir = iconPath(icon.dir, '');

    if (existsSync(dir)) {
      for (const file of readdirSync(dir).filter((name) => OWN_ICON.test(name))) {
        rmSync(join(dir, file));
      }
    }
  }
}

function install(): void {
  const binary = join(ROOT, 'target', 'debug', APP_ID);
  const icon = iconName();

  mkdirSync(dirname(entryPath), { recursive: true });
  writeFileSync(entryPath, desktopEntry(binary, icon));

  removeIcons();
  for (const file of ICON_FILES) {
    const target = iconPath(file.dir, `${icon}.${file.extension}`);

    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(join(ICONS, file.source), target);
  }

  refreshCaches();

  console.log(`Installed ${entryPath} and ${ICON_FILES.length} icons under ${join(DATA_HOME, 'icons', 'hicolor')}.`);
  console.log('Restart Harness Desktop to see the icon. Undo with: bun run desktop-entry -- --remove');
}

function remove(): void {
  if (existsSync(entryPath)) {
    rmSync(entryPath);
  }

  removeIcons();

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

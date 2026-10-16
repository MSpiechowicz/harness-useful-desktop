<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/lockup-dark.svg">
    <img src="docs/brand/lockup-light.svg" alt="harness desktop" width="430">
  </picture>
</p>

# Harness Desktop

**Your coding-agent CLI, in a native desktop window.** Harness Desktop is a native desktop app for coding-agent harness
CLIs. It runs on top of the CLI you already have installed: chat with the agent in a window, browse its sessions and branches, review the working-tree changes it made
and send your comments back, and read and manage its configuration and extensions. Everything is local: the app has
no telemetry, no accounts and stores no tokens, and credential files are never read.

https://github.com/user-attachments/assets/edf7e511-22ea-454c-988f-8e2639f7f818

<sub>A 73-second tour with sound: chat with live tool calls and subagents, approvals, sessions and branches, review
comments sent back to the agent, config and extensions, the Harness family and Harness Dashboard data, themes and
privacy. Made with the app's demo data. [Download the full-quality 4K video](https://raw.githubusercontent.com/MSpiechowicz/harness-useful-desktop/main/docs/video/harness-desktop-tour.mp4)
(54 MB).</sub>

## Features

- **Chat over RPC with approvals.** Streams the agent's replies, thinking and tool calls (the harness runs in
  `--mode rpc`), answers its approval and input dialogs, picks the model and thinking level, runs slash commands, and
  asks before any shell command you send.
- **Sessions and branches.** Lists a project's saved sessions, shows the branch tree of each, opens the transcript,
  resumes or forks.
- **Changes panel with review comments.** A graphical diff of the working tree (staged, unstaged, untracked). Comment
  on any line and send the comments to the agent as one prompt.
- **Config viewer.** Settings, MCP servers and profiles in one readable place, with secrets masked.
- **Extensions.** Plugins, marketplaces, packages, skills, MCP servers and themes, with install, enable and update
  actions run through the harness's own CLI and a dry-run preview where the CLI offers one.
- **Ecosystem tab and dashboard widgets.** Detects the companion tools and shows cost and token widgets from the local
  [Harness Dashboard](https://github.com/MSpiechowicz/harness-useful-dashboard) when it is installed.
- **Recommended setup.** An optional onboarding step (also **Run recommended setup** on the Ecosystem screen) that,
  after you confirm each part, installs the Harness Dashboard with its official installer, installs the
  [Harness Skills](https://github.com/MSpiechowicz/harness-useful-skills) (OMP as a plugin, PI as a package) and
  registers the dashboard as an MCP server in each detected harness's user `mcp.json`.
- **Project memory.** Your harnesses share a local memory of what was done, problems and fixes, and where things are,
  per repository. See [Project memory](#project-memory).
- **Share setup.** In Settings, export your marketplaces, plugins, packages and MCP servers as a `hud1:` string or a
  `.json` file (references and key names; env and header values are never shared), and import one on another machine. An import always shows a review of
  the exact commands and files, adds only what you tick after you confirm, and never removes or overwrites anything.
  Format: [docs/share-setup.md](docs/share-setup.md).
- **Command palette and shortcuts.** Keyboard-first navigation (`Ctrl/Cmd+K`, `Ctrl/Cmd+1..7`), plus a shortcuts
  overlay.
- **Themes.** Light, dark or follow the system, with a motion setting (full, reduced, off).
- **Auto-update.** Signed updates from GitHub releases, checked against a key built into the app. Builds without an
  updater key never contact the network.

## Supported harnesses

| Harness | Status |
| --- | --- |
| [Oh My Pi](https://omp.sh) (`omp`) | Supported. Verified against 18.6.1. |
| [Pi](https://github.com/earendil-works/pi) (`pi`) | Supported. Follows the PI documentation (v1.1.0); detected versions show as untested until a release is verified live. |
| Other harness CLIs | Planned. Adding one is an implementation of the `Harness` trait; see [docs/adding-a-harness.md](docs/adding-a-harness.md). Every new harness must also join Recommended setup (skills install and dashboard MCP). |

The app finds the CLI on `PATH` and in the usual install directories (bun, cargo, npm global, `~/.local/bin`, Homebrew), or you can
choose the executable yourself. It does not install harnesses.

## Project memory

The app bundles `harness-memory`, a small MCP server that gives every detected harness a shared memory. Agents search it
at the start of a task, save what they learn (outcomes, problems and fixes, where things live, decisions) before they
finish and keep it current, so the next session in the same repository starts better informed. It is on by default.

- **How it gets there.** At launch the app copies the server to a stable path (Linux `~/.local/bin/harness-memory`;
  macOS `~/Library/Application Support/com.mspiechowicz.harness-useful-desktop/bin/`; Windows
  `%LOCALAPPDATA%\com.mspiechowicz.harness-useful-desktop\bin\harness-memory.exe`) and adds a `harness-memory` entry to
  the user `mcp.json` of each detected harness. A file already named `harness-memory` in that folder is replaced by the
  app's copy. It never overwrites another entry, and an entry you removed is not added back.
- **What is stored.** The notes agents write, as text, plus the repository's name and path, and a small graph of related
  files and concepts. Strings pass a secret filter first, which is a heuristic: it does not catch everything. The
  database is not encrypted, is never exported by Share setup and is never sent anywhere by the app (what an agent reads
  from it goes to its model provider like the rest of its context). It is
  `memory/memory.sqlite3` in the app's local data folder: Linux `~/.local/share/com.mspiechowicz.harness-useful-desktop`
  (or `$XDG_DATA_HOME`), macOS `~/Library/Application Support/com.mspiechowicz.harness-useful-desktop`, Windows
  `%LOCALAPPDATA%\com.mspiechowicz.harness-useful-desktop`.
- **Notes are agent-written data.** Search results, full notes and the similar notes listed after a save say where each
  note came from (checkout and harness), and agents are told to check notes against the code and never to follow
  instructions inside one. The repository key is only a name: any agent with the server can read and write any
  repository's notes, so a same-title note from another checkout, or from an agent that names another repository by
  path, is reported instead of replaced.
- **Notes in new chats.** On the first message of a new chat, the app searches project memory with keywords from the
  message and puts up to five matching notes (at most 1.5 KiB, marked as data, not instructions) in front of it. Only
  this repository's notes written from this same folder are added: global notes, notes from another checkout of the
  repository and notes an agent saved by naming the repository's path are never added on their own (agents can still
  search them). The chat
  shows them as a collapsed chip, **Project memory · N notes**, that you can expand. Resumed chats, commands (`/…`),
  shell lines (`!…`) and later messages are left alone, and if the search finds nothing or takes more than 1.5 seconds the
  message goes out as you wrote it. The keywords stay on your machine; the notes are sent to your model provider with the
  message and saved by the harness in its session file, and harness terminal UIs show them as plain text. Switch
  **Add memory to new chats** off in Settings to stop this.
- **Turn it off.** Switch **Project memory** off in Settings: the app removes its entries from the harnesses and stops
  adding them (and stops adding notes to new chats). Your notes stay and come back when you switch it on again. Deleting the `harness-memory` entry from a
  harness's `mcp.json` turns it off for that harness only.
- **After uninstalling the app** the entries, the stable copy and the notes remain. To clean up, switch the setting off
  first (or delete the `harness-memory` entries from the harnesses' `mcp.json`), then delete the stable copy named
  above and the `memory` folder in the data folder.

Details and limits: [docs/security-privacy.md](docs/security-privacy.md#project-memory).

## Harness Useful family

Harness Desktop is one of a set of local tools for coding-agent harnesses:

| Project | What it is |
| --- | --- |
| [harness-useful-sidebar](https://github.com/MSpiechowicz/harness-useful-sidebar) | Live token charts and persistent session history for OMP in a tmux sidebar. |
| [harness-useful-desktop](https://github.com/MSpiechowicz/harness-useful-desktop) | This app: your coding-agent CLI in a native desktop window. |
| [harness-useful-dashboard](https://github.com/MSpiechowicz/harness-useful-dashboard) | Token usage and cost across your AI coding tools, in one local app. Its data powers the dashboard widgets here. |
| [harness-useful-skills](https://github.com/MSpiechowicz/harness-useful-skills) | Skills that take a natural-language request to reviewed, verified work. |

## Install

**macOS and Linux**

```sh
curl -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-desktop/main/install.sh | sh
```

On Linux this installs the AppImage as `~/.local/bin/harness-desktop` and adds an applications-menu entry. On macOS it
installs `~/Applications/Harness Desktop.app` (Apple Silicon or Intel), clears the download quarantine and gives the app
an ad-hoc signature, since the release is not notarized. The installer verifies the release checksum and installs
nothing without one. AppImages need FUSE 2: if the app does not start, install `libfuse2` (`libfuse2t64` on Ubuntu
24.04 and newer).

**Windows (PowerShell)**

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-desktop/main/install.ps1 | iex"
```

This verifies the checksum and runs the NSIS installer silently: a per-user install in `%LOCALAPPDATA%\Harness Desktop`
with a Start menu shortcut.

Installer options (environment variables): `HARNESS_DESKTOP_VERSION=v0.2.0` pins a release,
`HARNESS_DESKTOP_INSTALL_DIR=…` picks the Linux install directory and `HARNESS_DESKTOP_NO_SHORTCUT=1` skips the Linux
applications-menu entry.

You can also download an installer from the [releases page](https://github.com/MSpiechowicz/harness-useful-desktop/releases):

- Linux: AppImage, deb, rpm
- macOS: dmg (Apple Silicon and Intel)
- Windows: NSIS installer, MSI

Formats, update behaviour per format and the first-run warning on macOS and Windows (the installers are not
code-signed) are in
[What users get](docs/release-and-updater-keys.md#what-users-get) and
[unsigned installers](docs/release-and-updater-keys.md#macos-and-windows-unsigned-installers).

### Updating

The app checks GitHub releases and offers signed updates itself (Settings). Re-running the install command also
upgrades.

### Uninstalling

```sh
rm ~/.local/bin/harness-desktop                                              # Linux
rm ~/.local/share/applications/harness-desktop.desktop                       # Linux launcher
rm ~/.local/share/icons/hicolor/256x256/apps/harness-desktop.png             # Linux icon
rm -rf ~/Applications/"Harness Desktop.app"                                  # macOS
# project memory (optional; delete the harness-memory entries from the harnesses' mcp.json first, or switch the setting off):
rm ~/.local/bin/harness-memory                                               # Linux
# preferences, WebView data, logs and the memory notes (optional):
rm -rf ~/.config/com.mspiechowicz.harness-useful-desktop ~/.local/share/com.mspiechowicz.harness-useful-desktop  # Linux
rm -rf ~/Library/Application\ Support/com.mspiechowicz.harness-useful-desktop \
  ~/Library/Logs/com.mspiechowicz.harness-useful-desktop \
  ~/Library/WebKit/com.mspiechowicz.harness-useful-desktop \
  ~/Library/Caches/com.mspiechowicz.harness-useful-desktop                   # macOS
```

On Windows, uninstall **Harness Desktop** from Settings > Apps (or run `%LOCALAPPDATA%\Harness Desktop\uninstall.exe`),
then optionally delete `%APPDATA%\com.mspiechowicz.harness-useful-desktop` and
`%LOCALAPPDATA%\com.mspiechowicz.harness-useful-desktop` (the latter holds the project memory and its server copy). A deb or rpm install is removed with
`sudo apt remove harness-desktop` or `sudo dnf remove harness-desktop`.

## Privacy

Everything runs on your machine. No telemetry, no accounts, no stored tokens. Credential stores, databases, logs and
private keys are never read by the app (the project memory database belongs to the bundled `harness-memory` server, not
to the window), secrets in configuration are masked, and the only network requests the app makes itself are
the signed update check and a lookup of the companions' latest release numbers on GitHub; both can be turned off. Besides your harness CLI, the app runs the Harness Dashboard installer of Recommended setup (only after you confirm it) and, for the notes added to a new chat, the bundled `harness-memory` server (switch it off with **Add memory to new chats** in Settings). The app stores only preferences, window geometry, a few UI choices and a local log file; the notes your agents save to project memory are kept in a local database by the `harness-memory` server, and the notes added to the first message of a new chat go to your model provider with it. Details and the exact rules:
[docs/security-privacy.md](docs/security-privacy.md).

## Development

Prerequisites:

- Rust via [rustup](https://rustup.rs); `rust-toolchain.toml` selects the pinned toolchain (clippy, rustfmt included).
- [bun](https://bun.sh) 1.4+.
- Linux only: the Tauri system packages (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`,
  `patchelf`, `libxdo-dev`, `libssl-dev`, `libasound2-dev`, `build-essential`).

Commands:

```sh
bun install                        # install JS dependencies
bun run dev:ui                     # UI only, in a browser (http://localhost:1420)
bun run dev:mock                   # UI with the mock transport (no Rust needed)
bun run sidecar                    # build the harness-memory server the desktop crate bundles (dev and build run it)
bun run dev                        # full desktop app (tauri dev)
cargo test -p harness_business     # business-layer tests
cargo test -p harness_memory       # project memory server tests
bun run check                      # svelte-check / type check
bun run test                       # UI unit tests
bun run guard                      # file size, forbidden patterns, IPC contract
bun run dup                        # copy/paste (jscpd) detection
```

Also useful:

```sh
cargo test -p harness_business export_            # regenerate TypeScript bindings and mock fixtures
cargo build -p harness_business --bin fake_agent  # scripted agent used by the desktop-shell tests
HARNESS_REQUIRE_FAKE_AGENT=1 cargo test -p harness_desktop  # needs `bun run sidecar` first
bun run build:ui                                  # production UI bundle
bun run desktop-entry                             # Linux: register the dev build so Wayland shows its icon (-- --remove to undo)
```

Bundling (`bun run build`) needs the updater signing key; see
[docs/release-and-updater-keys.md](docs/release-and-updater-keys.md).

## Project layout

```text
business/      Rust crate harness_business: harness adapters, agent sessions, git, security. No Tauri dependency.
memory/        Rust crate harness_memory: the project memory MCP server (harness-memory), bundled as a sidecar.
src-tauri/     Thin Tauri shell: IPC commands, app state, capabilities (ACL), CSP, window and updater wiring.
ui/            Svelte 5 app: features, stores, IPC client, mock transport, generated bindings and fixtures.
scripts/       Guards (file size, forbidden patterns, IPC contract) and the release version script.
docs/          Architecture, adding a harness, security and privacy, the Share setup format, releases and updater keys.
.github/       CI and release workflows.
```

Read [docs/architecture.md](docs/architecture.md) for how the layers fit together.

## Contributing

- **Conventional Commits drive releases.** On `main`, `fix:` releases a patch, `feat:` a minor, and `feat!:` or a
  `BREAKING CHANGE:` footer a major. `chore:`, `docs:`, `test:` and similar do not release. semantic-release writes the
  changelog and the tag; see [docs/release-and-updater-keys.md](docs/release-and-updater-keys.md).
- **Guards fail the build.** Run `bun run guard` and `bun run dup` before pushing: authored code files stay within 500
  lines, security-sensitive patterns are rejected (shell spawns, `eval`, `innerHTML`, `--auto-approve`, `--no-ui`, and
  more), Rust and TypeScript IPC command lists match, and duplicated code is flagged.
- **Generated files.** Never edit `ui/src/lib/ipc/bindings/` or `ui/src/lib/ipc/mock/fixtures/`; regenerate them with
  `cargo test -p harness_business export_` and commit the result. CI fails when they are stale.
- **Tests.** The business crate carries the logic and its tests; the shell and UI stay thin.

## License

[GPL-3.0-or-later](LICENSE). Copyright (C) 2026 Maciej Spiechowicz.

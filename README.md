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
| Other harness CLIs | Planned. Adding one is an implementation of the `Harness` trait; see [docs/adding-a-harness.md](docs/adding-a-harness.md). |

The app finds the CLI on `PATH` and in the usual install directories (bun, cargo, npm global, `~/.local/bin`, Homebrew), or you can
choose the executable yourself. It does not install harnesses.

## Harness Useful family

Harness Desktop is one of a set of local tools for coding-agent harnesses:

| Project | What it is |
| --- | --- |
| [harness-useful-sidebar](https://github.com/MSpiechowicz/harness-useful-sidebar) | Live token charts and persistent session history for OMP in a tmux sidebar. |
| [harness-useful-desktop](https://github.com/MSpiechowicz/harness-useful-desktop) | This app: your coding-agent CLI in a native desktop window. |
| [harness-useful-dashboard](https://github.com/MSpiechowicz/harness-useful-dashboard) | Token usage and cost across your AI coding tools, in one local app. Its data powers the dashboard widgets here. |

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
# preferences, WebView data and logs (optional):
rm -rf ~/.config/com.mspiechowicz.harness-useful-desktop ~/.local/share/com.mspiechowicz.harness-useful-desktop  # Linux
rm -rf ~/Library/Application\ Support/com.mspiechowicz.harness-useful-desktop \
  ~/Library/Logs/com.mspiechowicz.harness-useful-desktop \
  ~/Library/WebKit/com.mspiechowicz.harness-useful-desktop \
  ~/Library/Caches/com.mspiechowicz.harness-useful-desktop                   # macOS
```

On Windows, uninstall **Harness Desktop** from Settings > Apps (or run `%LOCALAPPDATA%\Harness Desktop\uninstall.exe`),
then optionally delete `%APPDATA%\com.mspiechowicz.harness-useful-desktop` and
`%LOCALAPPDATA%\com.mspiechowicz.harness-useful-desktop`. A deb or rpm install is removed with
`sudo apt remove harness-desktop` or `sudo dnf remove harness-desktop`.

## Privacy

Everything runs on your machine. No telemetry, no accounts, no stored tokens. Credential stores, databases, logs and
private keys are never read, secrets in configuration are masked, and the only network requests the app makes itself are
the signed update check and a lookup of the companions' latest release numbers on GitHub; both can be turned off. The app stores only preferences, window geometry, a few UI choices and a local log file. Details and the exact rules:
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
bun run dev                        # full desktop app (tauri dev)
cargo test -p harness_business     # business-layer tests
bun run check                      # svelte-check / type check
bun run test                       # UI unit tests
bun run guard                      # file size, forbidden patterns, IPC contract
bun run dup                        # copy/paste (jscpd) detection
```

Also useful:

```sh
cargo test -p harness_business export_            # regenerate TypeScript bindings and mock fixtures
cargo build -p harness_business --bin fake_agent  # scripted agent used by the desktop-shell tests
HARNESS_REQUIRE_FAKE_AGENT=1 cargo test -p harness_desktop
bun run build:ui                                  # production UI bundle
bun run desktop-entry                             # Linux: register the dev build so Wayland shows its icon (-- --remove to undo)
```

Bundling (`bun run build`) needs the updater signing key; see
[docs/release-and-updater-keys.md](docs/release-and-updater-keys.md).

## Project layout

```text
business/      Rust crate harness_business: harness adapters, agent sessions, git, security. No Tauri dependency.
src-tauri/     Thin Tauri shell: IPC commands, app state, capabilities (ACL), CSP, window and updater wiring.
ui/            Svelte 5 app: features, stores, IPC client, mock transport, generated bindings and fixtures.
scripts/       Guards (file size, forbidden patterns, IPC contract) and the release version script.
docs/          Architecture, adding a harness, security and privacy, releases and updater keys.
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

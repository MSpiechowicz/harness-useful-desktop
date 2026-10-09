# Harness Desktop

Harness Desktop is a native desktop app for coding-agent harness CLIs. It runs on top of the CLI you already have
installed: chat with the agent in a window, browse its sessions and branches, review the working-tree changes it made
and send your comments back, and read and manage its configuration and extensions. Everything is local: the app has
no telemetry, no accounts and stores no tokens, and credential files are never read.

<!-- screenshots: added after the visual pass -->

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

## Install

Download the installer for your OS from the [releases page](https://github.com/MSpiechowicz/harness-useful-desktop/releases):

- Linux: AppImage, deb, rpm
- macOS: dmg (Apple Silicon and Intel)
- Windows: NSIS installer, MSI

Formats, update behaviour per format and the first-run warning on macOS and Windows (the installers are not
code-signed) are in
[What users get](docs/release-and-updater-keys.md#what-users-get) and
[unsigned installers](docs/release-and-updater-keys.md#macos-and-windows-unsigned-installers).

## Privacy

Everything runs on your machine. No telemetry, no accounts, no stored tokens. Credential stores, databases, logs and
private keys are never read, secrets in configuration are masked, and the only network request the app makes itself is
the signed update check. The app stores only preferences, window geometry, a few UI choices and a local log file. Details and the exact rules:
[docs/security-privacy.md](docs/security-privacy.md).

## Development

Prerequisites:

- Rust via [rustup](https://rustup.rs); `rust-toolchain.toml` selects the pinned toolchain (clippy, rustfmt included).
- [bun](https://bun.sh) 1.4+.
- Linux only: the Tauri system packages (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`,
  `patchelf`, `libxdo-dev`, `libssl-dev`, `build-essential`).

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

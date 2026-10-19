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

- **Chat with approvals.** Streams the agent's replies, thinking and tool calls (OMP and PI run in `--mode rpc`,
  OpenCode as a local `opencode serve` the app talks to on this machine), answers its approval and input dialogs, picks
  the model and thinking level (and OpenCode's agent), runs slash commands, and asks before any shell command you
  send. Chat with OpenCode is a preview.
- **Sessions and branches.** Lists a project's saved sessions, shows the branch tree of each, opens the transcript,
  resumes or forks. OpenCode sessions are linear: the app lists them, opens the transcript and deletes them through
  the `opencode` CLI, with no branch tree, and resumes or forks them in a chat.
- **Changes panel with review comments.** A graphical diff of the working tree (staged, unstaged, untracked). Comment
  on any line and send the comments to the agent as one prompt.
- **Config viewer.** Settings, MCP servers and profiles in one readable place, with secrets masked.
- **Extensions.** Plugins, marketplaces, packages, skills, commands, agents, MCP servers and themes, with install,
  enable and update actions run through the harness's own CLI and a dry-run preview where the CLI offers one. OpenCode's
  extensions are read-only.
- **Ecosystem tab and dashboard widgets.** Detects the companion tools and shows cost and token widgets from the local
  [Harness Dashboard](https://github.com/MSpiechowicz/harness-useful-dashboard) when it is installed.
- **Recommended setup.** An optional onboarding step (also **Run recommended setup** on the Ecosystem screen) that,
  after you confirm each part, installs the Harness Dashboard and Harness Memory with their official installers, installs the
  [Harness Skills](https://github.com/MSpiechowicz/harness-useful-skills) (OMP as a plugin, PI as a package, not yet
  for OpenCode) and registers the dashboard as an MCP server in each detected harness: in its user `mcp.json` for OMP
  and PI, in the `mcp` object of the global `opencode.json` or `opencode.jsonc` for OpenCode. An OpenCode config file
  with comments is never rewritten, so the app shows the entry to paste instead. The same step can register the app's own
  `harness-desktop` MCP server (for hooks) in each harness.
- **Project memory.** Your harnesses share a local memory of what was done, problems and fixes, and where things are,
  per repository. See [Project memory](#project-memory).
- **Hooks.** Small rules that add context to a prompt, send a follow-up after the agent finishes or run a command when
  something happens in a chat, with ready-made ones for Harness Memory and the dashboard. See [Hooks](#hooks).
- **Share setup.** In Settings, export the marketplaces, plugins, packages and MCP servers of OMP and PI as a `hud1:`
  string or a `.json` file (references and key names, env and header values are never shared), and import one on another
  machine. OpenCode is not covered yet. An import always shows a review of
  the exact commands and files, adds only what you tick after you confirm, and never removes or overwrites anything.
  Format: [docs/share-setup.md](docs/share-setup.md).
- **Command palette and shortcuts.** Keyboard-first navigation (`Ctrl/Cmd+K`, `Ctrl/Cmd+1..9`), plus a shortcuts
  overlay.
- **Themes.** Light, dark or follow the system, with a motion setting (full, reduced, off).
- **Auto-update.** Signed updates from GitHub releases, checked against a key built into the app. Builds without an
  updater key never contact the network.

## Supported harnesses

| Harness | Status |
| --- | --- |
| [Oh My Pi](https://omp.sh) (`omp`) | Supported. Verified against 18.6.1. |
| [Pi](https://github.com/earendil-works/pi) (`pi`) | Supported. Tested with PI 1.1. Other versions show as untested. |
| [OpenCode](https://opencode.ai) (`opencode`) | Partly supported. Works: detection (also in `~/.opencode/bin`), the configuration (JSON with comments, secrets masked), extensions (skills, commands, agents, plugins, MCP servers, themes, read-only), sessions (list, transcript, delete) and the dashboard and memory MCP entries in Recommended setup. Versions 1.18 and 1.19 are supported, others show as untested. Chat works as a preview (see [docs/security-privacy.md](docs/security-privacy.md#opencode-chat) for how the app runs the server). No Harness Skills install and no Share setup yet. |
| Other harness CLIs | Planned. Adding one is an implementation of the `Harness` trait; see [docs/adding-a-harness.md](docs/adding-a-harness.md). Every new harness must also join Recommended setup (skills install and dashboard MCP). |

The app finds the CLI on `PATH` and in the usual install directories (bun, cargo, npm global, `~/.local/bin`, `~/.opencode/bin`, Homebrew), or you can
choose the executable yourself. It does not install harnesses.

## Project memory

[Harness Memory](https://github.com/MSpiechowicz/harness-useful-memory) (`harness-memory`) is a small MCP server that gives
every detected harness a shared memory. Agents search it at the start of a task, save what they learn (outcomes,
problems and fixes, where things live, decisions) before they finish and keep it current, so the next session in the
same repository starts better informed. The switch is on by default; the server is a separate program that you install.

- **How it gets there.** The app does not bundle the server. Recommended setup (also reachable from the Ecosystem
  tab) offers to install it and runs its official installer (`install.sh`, or `install.ps1` on Windows, which verify
  the release checksum) only after you confirm. The server lands in `~/.local/bin/harness-memory` (Linux, macOS) or
  `%LOCALAPPDATA%\Programs\harness-memory\harness-memory.exe` (Windows). While the switch is on and the server is
  installed, the app adds a `harness-memory` entry that runs `harness-memory mcp` to the user `mcp.json` of each
  detected OMP and PI, and to the `mcp` object of OpenCode's global `opencode.json` or `opencode.jsonc`, at launch. It
  never overwrites another entry, never rewrites an OpenCode file that has comments, and an entry you removed is not
  added back.
- **What is stored.** The notes agents write, as text, plus the repository's name and path, and a small graph of related
  files and concepts. Strings pass a secret filter first, which is a heuristic: it does not catch everything. The
  database is not encrypted, is never exported by Share setup and is never sent anywhere by the app, which does not
  open it (what an agent reads from it goes to its model provider like the rest of its context). The server keeps it
  at `harness-memory/memory.sqlite3` in your data folder: Linux `~/.local/share` (or `$XDG_DATA_HOME`), macOS
  `~/Library/Application Support`, Windows `%LOCALAPPDATA%` (`HARNESS_MEMORY_DIR` overrides it). On its first run the
  server copies the database that app versions up to 0.16 kept under
  `com.mspiechowicz.harness-useful-desktop/memory` in the app's local data folder; the old file is left in place.
- **Notes are agent-written data.** Search results, full notes and the similar notes listed after a save say where each
  note came from (checkout and harness), and agents are told to check notes against the code and never to follow
  instructions inside one. The repository key is only a name: any agent with the server can read and write any
  repository's notes, so a same-title note from another checkout, or from an agent that names another repository by
  path, is reported instead of replaced.
- **Turn it off.** Switch **Project memory** off in Settings: the app removes its entries from the harnesses and stops
  adding them. Your notes stay and come back when you switch it on again. Deleting the `harness-memory` entry from a
  harness's `mcp.json` (OpenCode: its `mcp` object) turns it off for that harness only.
- **Coming from an app version up to 0.16.** On macOS and Windows those versions kept a copy of the server in the app's
  local data folder (`bin/harness-memory[.exe]`). Once the server is installed, the next launch points the app's own
  entries at it and deletes that copy when nothing else uses it (a symlink is never removed). On Linux the path is
  unchanged.
- **After uninstalling the app** the entries, the server and the notes remain. To clean up, switch the setting off
  first (or delete the `harness-memory` entries from the harnesses' `mcp.json` and OpenCode's `mcp` object), then remove the server as described in
  the [harness-useful-memory README](https://github.com/MSpiechowicz/harness-useful-memory#uninstalling) and delete the
  `harness-memory` folder in the data folder named above.

Details and limits: [docs/security-privacy.md](docs/security-privacy.md#project-memory).

## Hooks

Hooks are small rules the app applies in your chats with OMP, PI and OpenCode. Open the
Hooks view (after Memory in the rail) to add, edit, switch off and delete them. A hook fires on one event:

- **Session start**: the first message of a new chat. **Every message**: each message you send.
- **Tool end**: after a tool call, optionally only for named tools. **Turn end**: when the agent finished your request.
- **Session end**: when the chat closes.

A hook can add a text in front of your message, send a follow-up message after the agent finished (at most one per request,
and only after a request in which a tool call succeeded), or run a shell command whose output is used the same way. The
chat shows what a hook added as a card, and a message a hook sent as sent by the hook.

- **Approval.** A hook you create or edit in the app runs at once. A hook an agent adds or changes is pending in the view
  and never runs until you approve that exact version, with its text or command and what changed in front of you. An
  agent can remove only hooks it created.
- **Commands run a shell.** A command hook runs through `/bin/sh -c` (Linux, macOS) or `cmd.exe` (Windows) with your
  privileges, in the project folder, with a 10 second default limit (at most 30 seconds). It gets the event, project,
  session, tool name and changed paths as `HARNESS_HOOK_*` variables, never a tool's result. Its output (16 KiB at most,
  secrets masked as far as the filter can) is added to the prompt or sent as the follow-up, and a failure shows a short
  notice in the chat without the command or output.
- **Harness family hooks.** Hooks that ask agents to use the other Harness tools: search Harness Memory first and save
  what they learned, and check limits with the dashboard before long work. The app creates them once when the tool is
  available and adds them again after it installs the tool. **More actions → Harness family hooks** in the Hooks view adds
  or updates them. A hook you delete is not created again at launch.
- **Agents manage hooks over MCP.** The app registers itself as the `harness-desktop` MCP server (`harness-desktop mcp`,
  tools `hook_list`, `hook_get`, `hook_add`, `hook_update`, `hook_remove`) in the user `mcp.json` of each detected harness
  at launch (release builds), like Harness Memory: it never overwrites another entry, follows the app when it moves, and an
  entry you removed is not added back. Recommended setup shows the state per harness and adds it on request.
- **Where they are stored.** One JSON file per hook in `harness-hooks/hooks` in your data folder (Linux `~/.local/share`
  or `$XDG_DATA_HOME`, macOS `~/Library/Application Support`, Windows `%LOCALAPPDATA%`; `HARNESS_HOOKS_DIR` overrides it),
  in plain text. Your approvals are kept in the app's config folder. What a hook adds to a prompt goes to your model
  provider with it and is saved by the harness in its session file. Format: [docs/hooks-format.md](docs/hooks-format.md).
- **After uninstalling the app** the hook files and the `harness-desktop` entries remain. Remove the entries from the
  harnesses' `mcp.json` (OpenCode: its `mcp` object) and delete the `harness-hooks` folder.

Details and limits: [docs/security-privacy.md](docs/security-privacy.md#hooks).

## Harness Useful family

Harness Desktop is one of a set of local tools for coding-agent harnesses:

| Project | What it is |
| --- | --- |
| [harness-useful-sidebar](https://github.com/MSpiechowicz/harness-useful-sidebar) | Live token charts and session history for Oh My Pi in a tmux sidebar and for Claude Code in a native pane. |
| [harness-useful-desktop](https://github.com/MSpiechowicz/harness-useful-desktop) | This app: your coding-agent CLI in a native desktop window. |
| [harness-useful-dashboard](https://github.com/MSpiechowicz/harness-useful-dashboard) | Token usage and cost across your AI coding tools, in one local app. Its data powers the dashboard widgets here. |
| [harness-useful-skills](https://github.com/MSpiechowicz/harness-useful-skills) | Skills that take a natural-language request to reviewed, verified work. |
| [harness-useful-memory](https://github.com/MSpiechowicz/harness-useful-memory) | A local MCP server that gives your coding agents one shared project memory. Installed from Recommended setup. |

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
upgrades. With **Update extensions automatically** on (the default), the app also keeps your installed Harness Memory and
Harness Dashboard current by running their official installers, and you can turn that off in Settings under Updates.

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
`%LOCALAPPDATA%\com.mspiechowicz.harness-useful-desktop` A deb or rpm install is removed with
`sudo apt remove harness-desktop` or `sudo dnf remove harness-desktop`. Harness Memory and its notes are a separate
install and stay; see [Project memory](#project-memory) for removing them.

## Privacy

Everything runs on your machine. No telemetry, no accounts, no stored tokens. Credential stores, databases, logs and
private keys are never read by the app (the project memory database belongs to the `harness-memory` server, not
to the window), secrets in configuration are masked, and the only network requests the app makes itself are
the signed update check and a lookup of the companions' latest release numbers on GitHub; both can be turned off. Besides your harness CLI, the app runs the Harness Dashboard and Harness Memory installers (only after you confirm them; they download from GitHub) and the shell commands of the hooks you wrote or approved. The app stores only preferences, window geometry, a few UI choices, your hook approvals and a local log file; the hook files are kept in your data folder, and the notes your agents save to project memory are kept in a local database by the `harness-memory` server. What a hook adds to a prompt goes to your model provider with it. Details and the exact rules:
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

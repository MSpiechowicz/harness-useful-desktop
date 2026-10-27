# Share setup

**Share setup** (Settings, section "Share setup") copies the extensions and MCP servers of your harnesses to another
machine. **Export** produces a `hud1:` string and a `.json` file; **Import** reads either, shows a review of exactly
what would happen, and applies only the items you tick after you confirm. This page is the format reference. For the
guarantees around it see [security-privacy.md](security-privacy.md#shared-setups).

What a setup holds is references: marketplace sources, plugin ids, package sources, MCP commands, arguments and URLs,
and the names (never the values) of the environment variables and headers an MCP server needs. Values of `env`,
`headers`, `auth` and `oauth` are never read into a setup. Free-form text (MCP arguments, URLs, sources) is checked with
best-effort secret and path heuristics, and host names appear in sources and URLs by nature; the import review prints
every command line so you can check what is shared. See [What is never exported](#what-is-never-exported).

## Flows

**Export.** The dialog lists what Rust finds in the harnesses' own user files, per installed harness, all ticked,
plus a list of what is left out and why. Untick what you do not want to share, then **Copy** the `hud1:` string or
**Save** a file (a native save dialog, default name `harness-setup-<date>.json`). The webview sends only the ids of the
ticked items; Rust rebuilds the bundle from disk each time. A string over 64 KiB is refused with a pointer to the file.

**Import.** Paste a `hud1:` string or the JSON of a file, or pick a file in a native dialog. Nothing runs on paste: the
app builds a review first. Each row shows its kind, name, status and the exact steps (the harness command line, or the
`mcp.json` file and the entry that would be merged). You tick items, press the confirm button, and the app applies them.
A new preview replaces the previous review. The review is used up when an apply for it starts (see
[Applying](#applying)).

Entry point: the Settings section only. Nothing is added to the Extensions menu.

## What an import can and cannot do

An import **adds**, and for MCP servers it can also **replace**. These are the only actions it can produce, and only in the user scope:

| Item | What runs |
| --- | --- |
| Marketplace (OMP) | `omp plugin marketplace add <source>` |
| Plugin (OMP) | `omp plugin install <id> --scope user` |
| Package (PI) | `pi install <source>` (global, no `--local`) |
| Plugin (OpenCode) | `opencode plugin <name>[@<version>] -g` |
| Marketplace (Claude Code) | `claude plugin marketplace add <source> --scope user --json` |
| Plugin (Claude Code) | `claude plugin install <id> --scope user --json` |
| MCP server | One atomic merge of the entry into the harness's user `mcp.json` (OpenCode: the `mcp` object of its global config file). A server of the same name with another definition, if you tick it, is replaced in place by the same merge |
| MCP server (Claude Code) | `claude mcp add-json --scope user <name> <json>`, one run per server (the app never writes `~/.claude.json`). A replace runs `claude mcp remove --scope user <name>` first |
| MCP server (Codex) | `codex mcp add <name> …`, one run per server (the app never writes `config.toml`). A replace runs `codex mcp remove <name>` first |

It never uninstalls, upgrades, downgrades, or touches project scope, app preferences or harness settings.
Marketplaces, plugins and packages are **add-only**: something already present is left alone, and a marketplace of the
same name with another source is a `conflict`. The one thing an import can overwrite is an MCP server you tick in the
review (status `differentDefinition`): for an http/sse server at the same endpoint only its URL is replaced and its other settings
are kept; otherwise (a stdio server, another endpoint) the whole entry is replaced and nothing of the old one is kept, see
[MCP servers](#mcp-servers).
Versions in a bundle are informational: the latest version is installed (a PI source keeps any pin already written in
the source string). Plugin enabled/disabled state is not copied.

An add can still run code: a CLI install of a marketplace, plugin or package may run install scripts (npm lifecycle
scripts, a plugin's setup) with your privileges, and an MCP server entry is a program or a remote service the harness
will use. The review shows the exact command line or `mcp.json` entry of every item before you confirm.

The app builds every command itself from validated fields; the webview sends only a plan id and item ids, never a
command, a path or a bundle. The `mcp.json` path comes from the harness's own paths, never from the bundle.

## The `hud1:` string

```text
hud1: + base64url(no padding) of zlib( compact JSON )
```

- The prefix is case-sensitive. Surrounding whitespace is trimmed and ASCII whitespace inside the string is ignored, so
  a copy that wrapped over several lines still works. Input that starts with `{` is read as JSON directly.
- zlib's Adler-32 checksum **detects corruption** (a truncated or mangled string fails). It does **not** prove who made
  the setup. **Bundles are not signed.** Treat a shared setup like a shared script: import only what you trust, and read
  the review.
- A `.json` file holds the same document, pretty-printed (an optional BOM and a saved `hud1:` string are accepted too).

### Limits

Enforced when decoding, before the next stage allocates. A violation rejects the whole input with a message that never
echoes it.

| Limit | Value |
| --- | --- |
| Pasted text | 128 KiB |
| Compressed payload after base64 decoding | 96 KiB |
| Decompressed JSON (read through a capped reader, a zip-bomb guard) | 256 KiB |
| JSON file | 256 KiB |
| Export string | 64 KiB (larger setups must be saved as a file) |
| Harness sections | 16 |
| Items per kind and harness | 200 |
| Items in total | 500 |
| Text field (name, source, id, version) | 512 characters |
| MCP command | 256 characters |
| MCP arguments | 64 entries, 1024 characters each |
| MCP URL | 2048 characters |
| `envKeys` / `headerKeys` | 64 each, 128 characters per name |
| JSON nesting | serde_json's recursion limit (128) |

## Schema v1

```json
{
  "format": "harness-useful-desktop/setup",
  "version": 1,
  "app": "0.15.0",
  "createdAt": "2026-10-15",
  "harnesses": {
    "omp": {
      "marketplaces": [{ "name": "acme-tools", "source": "acme/tools" }],
      "plugins": [{ "id": "formatter@acme-tools", "marketplace": "acme-tools", "version": "1.2.0" }],
      "mcpServers": [
        { "name": "docs", "transport": "stdio", "command": "docs-server", "args": ["--stdio"], "envKeys": ["DOCS_TOKEN"] },
        { "name": "search", "transport": "http", "url": "https://mcp.example.com/mcp", "headerKeys": ["Authorization"] }
      ]
    },
    "pi": { "packages": [{ "source": "npm:some-pi-package@1.2.0" }] }
  }
}
```

Empty lists are omitted when written. Top level:

| Field | Type | Meaning |
| --- | --- | --- |
| `format` | string | Exactly `harness-useful-desktop/setup`. |
| `version` | integer | `1`. |
| `app` | string, optional | App version that wrote it, semver-like (`0.15.0`, `v2.1`, `1.0.0-beta.1`). Anything else is dropped. |
| `createdAt` | string, optional | Export date, `YYYY-MM-DD` with a month of 01 to 12 and a day of 01 to 31. Anything else is dropped. |
| `harnesses` | object | One section per harness id (`omp`, `pi`, `opencode`, `claude`). |

A harness section holds up to four arrays; which ones a harness accepts depends on what it supports (OMP:
marketplaces, plugins, MCP servers; PI: packages, MCP servers; OpenCode: plugins, MCP servers; Claude Code:
marketplaces, plugins, MCP servers). A Claude Code plugin is always `name@marketplace`. An OpenCode
plugin is an npm package name (`@scope/name`) with the version, range or tag its config pins, never a marketplace. An
OpenCode server uses the same fields as the others (`local` is shared as stdio, `remote` as http) and is written as
`{"type": "local" | "remote", ...}` with `enabled` set.

| Array | Field | Type | Meaning |
| --- | --- | --- | --- |
| `marketplaces` | `name` | string | `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`. |
| | `source` | string | A GitHub `owner/repo` or an `https://` URL on a host name. |
| `plugins` | `id` | string | `name@marketplace` or an npm package name. |
| | `marketplace` | string, optional | Must match the `@marketplace` part of `id`. |
| | `version` | string, optional | Informational. |
| `packages` | `source` | string | `npm:…`, `git:…` or `https://…`, with any pin it had (see [Validation](#validation-of-every-item)). |
| | `version` | string, optional | Informational. |
| `mcpServers` | `name` | string | `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, never `harness-dashboard`, `harness`, `harness-memory`, `harness-desktop`, `harness-useful-skills` or `useful_skills` (reserved, any case). |
| | `transport` | `stdio`, `http`, `sse` | How the harness talks to it. |
| | `command` | string | Stdio only: a bare program name (`^[A-Za-z0-9][A-Za-z0-9._+-]*$`: no directory, no flag, not a shell). |
| | `args` | string array | Stdio only. |
| | `url` | string | `http`/`sse` only: `http://` or `https://`, no user info, no `${…}` or `$NAME`. |
| | `envKeys`, `headerKeys` | string arrays | Names only, `^[A-Za-z_][A-Za-z0-9_-]{0,127}$`. |

A stdio server may not carry a `url`; an `http`/`sse` server may not carry `command` or `args`.

### Validation of every item

Each item is parsed and checked on its own, so one bad item becomes `invalid` and the rest still import. A refused value
is never echoed back. Besides the shapes above:

- **Display safety.** Every displayed field is rejected if it holds control characters, any Unicode format character
  (category Cf: bidirectional overrides, isolates and marks, zero-width characters, the soft hyphen, the byte-order
  mark, tag characters), a line or paragraph separator (Zl, Zp), or another character that renders as nothing: U+034F,
  the Hangul fillers (U+115F, U+1160, U+3164, U+FFA0), the Khmer vowels U+17B4 and U+17B5, the Mongolian variation
  selectors (U+180B to U+180F), the blank Braille pattern (U+2800) and the variation selectors (U+FE00 to U+FE0F,
  U+E0100 to U+E01EF). Fields render as text only.
- **Common checks for sources and ids.** Every CLI target (marketplace source, plugin id, package source) passes the same
  `cli_target` check as the Extensions actions (only letters, digits and `@ . _ / : + = , -`, no leading `-`, 512
  characters at most) plus an import-only check: no `..` segment, no leading `/` or `~`, no `file:` or `link:`, no drive
  letter, no backslash, no credentials in the source (`https://user:token@host`, `git:token@host`; the conventional
  `git@` is allowed), nothing the redactor would hide.
- **Nothing that reads as an option.** No source or spec may start with `-`, not right after `npm:` or `git:`, not after
  an `@` or `#` ref, and no host or authority may start with `-`. Every URL or git host must be a plain host name
  (letters, digits, dots and inner hyphens), and a port must be a number.
- **Marketplace sources:** a GitHub `owner/repo`, or an `https://` URL on a host name.
- **PI package sources:** `npm:`, `git:` or `https://` only; a bare `ssh://` or `http://` source is refused.
  - `npm:` takes a registry name with an optional `@version`, range or tag (`pkg`, `@scope/pkg@1.2.0`, `pkg@latest`),
    `github:owner/repo`, or an `https://`, `git+https://` or `git+ssh://` URL on a host name (a user in the URL counts
    as credentials, so `git+ssh://git@host/…` is refused).
  - Refused as a local path: anything after `npm:` or after `name@` that starts with `.`, `/`, `~`, `\` or
    `<letter>:`, `file:`, `link:`, `git+file:`, and tarball or archive files (`.tgz`, `.tar`, `.tar.gz`, `.zip`) that are
    not URLs.
  - Refused as unsupported: `http:`, `git+http:`, `git:` forms, other transports, remote tarballs, and a bare
    `npm:owner/repo`.
  - `git:` takes `host/owner/repo`, `git@host:owner/repo` or an `https://` URL; `git:file:…` is a local path, and any
    other `://` form (`ssh://`, `git+ssh://`, `http://`) is refused.
- **Plugin ids:** `name@marketplace` or an npm package name, never a path or URL; `marketplace`, if present, matches the
  `@marketplace` part. Claude Code takes `name@marketplace` only: it would install a bare name from whichever
  registered marketplace lists it.
- **MCP server names:** the pattern above. `harness-dashboard` (and `harness`, the dashboard's name before it was
  registered under that one), `harness-memory`, `harness-desktop`, `harness-useful-skills` and `useful_skills` (its name
  before Harness Skills 0.4.0), in any case, are reserved: Recommended setup installs and registers the first, the app
  installs and registers the second (project memory) and the third (its hooks and routines server), and the Harness
  Skills installer registers the last two (OpenCode, Codex). None is ever exported, and an imported one is refused (for
  every harness, as a bad name).
- **MCP commands:** a bare program name, never a path and never a shell interpreter (`sh`, `bash`, `zsh`, `dash`,
  `fish`, `cmd`, `pwsh`, `powershell`, with or without a directory or `.exe`). Never `harness-dashboard` or
  `harness-memory` either (any case, with or without `.exe`): together with the reserved names, an import can neither
  add the app's two entries nor shadow them with a server of another name that runs the same program.
- **Environment references.** A harness expands `$NAME` and `${NAME}` from its own environment, so a reference in a
  received setup could hand a variable of this machine to the setup's author. A URL may hold none. An argument may only
  reference a name the same item lists in `envKeys`, as a plain `$NAME` or `${NAME}` (and then the server is written
  disabled); an undeclared name and any other `${…}` form (a default such as `${NAME:-x}`, an unclosed brace) are
  refused. A `$` that does not start a name (`5$`, `$1`) is not a reference.
- **Claude Code servers:** no `${` in the command, an argument or the URL, not even a declared `${NAME}`. Claude Code
  expands `${NAME}` and `${NAME:-default}` there from its environment, and it has no way to write a server switched
  off. A plain `$NAME` is passed on as written.
- **MCP URLs:** `http://` or `https://` with a host, no whitespace, no user info, no `${…}` or `$NAME` (even a declared
  one), no secret query value (`?api_key=…`).
- **Machine paths.** On export, an MCP argument that holds a path of this machine is dropped only in two cases, and the
  item then carries the note "arguments with a path of this machine are not shared" (the importing machine uses the
  program's default for that option): an argument with its own value (`--image-dir=/home/alice/x`) is dropped alone, and
  a path on its own is dropped together with the `--option` right before it (`--image-dir /home/alice/x`) only when
  that option name ends in `dir`, `directory`, `path`, `file`, `folder`, `root`, `home`, `cwd` or `config`. Any other
  machine-path argument (a script, a positional path, a path after a flag that takes none) cannot be dropped without
  changing what the server does, so the whole server is left out (reason `LocalPath`). A server whose URL or command
  holds such a path is left out whole too. An import compares arguments exactly, so a server exported without its
  paths differs from one a machine configured with its own: it is offered as `differentDefinition` and may be ticked
  to replace it. The paths detected are: an
  absolute, home or drive path right after `=` (`--root=/home/alice`, `--config=~/x`), an absolute path right after
  `@`, a system or user directory (`home`, `users`, `root`, `mnt`, `var`, `tmp`, `opt`, `srv`, `media`, `private`)
  after `,` or `;` and slashes, or after `:` and one or three or more slashes (`PATH=/usr/bin:/home/alice/bin`,
  `sqlite:///home/alice/x.db`; two slashes after `:` start a URL host and do not match). On import, sources, ids and
  the MCP command are refused for a local path; the arguments and URL of an MCP server are not checked for paths, so
  read them in the review. Path detection is a heuristic on free-form text: an unusual form can slip through export.
- **Secrets.** A value the redactor would hide (credential formats, secret-named assignments, high-entropy tokens, URL
  query secrets, `!command`) makes the item `invalid`. In MCP arguments the rule is stricter. A secret flag followed by
  anything but a `$NAME` reference makes the server non-exportable and the import refuses it. Secret flags: `-p` or
  `-P` with a separate value (`-p hunter2`; a port number such as `-p 8080` is fine) or an attached one that is not all
  digits (`-phunter2`; a single-dash word like `-port` counts too, a known false positive), `--password`, `--pass`,
  `--pwd`, `--pat`, `--key`, `--token`, `--api-key` and the other names the redactor treats as secret, as `--flag value`
  or `--flag=value`, whatever the value (a number is no exception, apart from the `-p`/`-P` port). A `user:password`
  value after `-u`, `--user`, `--auth`, `--basic-auth` or `--proxy-user` is refused too, unless the password is a
  `$NAME` reference. Detection of secrets in free-form arguments is best effort: an unusual form may not be caught.
- **Versions and dates:** `app` and every `version` field must look like a semver (`1.2.3`, `v2.1`,
  `1.0.0-beta.1+build.5`); a bad `version` makes its item `invalid`, a bad `app` or `createdAt` is dropped.

## What is never exported

Left out of the file and listed in the export dialog by name and reason (never by value):

| Never shared | Why |
| --- | --- |
| Values of `env`, `headers`, `auth`, `oauth` | Secrets. Only key names travel. This is structural: the values are never copied into a setup. |
| MCP `cwd` and any field outside the allowlist (`type`, `command`, `args`, `url`, key names) | Paths and settings of this machine. |
| MCP arguments that hold a machine path | An `--option=/path` argument is dropped alone; a path on its own is dropped with the `--option` before it when that name ends in `dir`, `directory`, `path`, `file`, `folder`, `root`, `home`, `cwd` or `config`. The server is still shared, with a note. Any other machine path (a script, a positional path, a path after a flag that takes none) leaves the whole server out. |
| MCP servers whose command, arguments or URL hold a secret or credentials, or whose command or URL holds a machine path | The whole server is left out. Heuristics (see [Validation](#validation-of-every-item)), not a proof. |
| MCP servers with an undeclared `${VAR}` / `$NAME` reference, a shell command, a non-bare command, or a `harness-dashboard` / `harness-memory` command under another name | Would hand an environment variable to the setup's author, run a shell, or shadow an app entry on the other machine. |
| The `harness-dashboard` server and its older name `harness`, and the `harness-desktop` server (any letter case) | Recommended setup and the app install them. Skipped without a row in the list; an import refuses these names. |
| The `harness-memory` server (any letter case) and the project memory database | The app installs and registers the server itself, and the notes are local to this machine. Skipped without a row in the list; an import refuses that name. |
| The `harness-useful-skills` server and its name before Harness Skills 0.4.0, `useful_skills` (any letter case) | The Harness Skills MCP server, registered by its installer (OpenCode, Codex). Skipped without a row in the list. An import refuses both names for every harness. |
| Local or unknown PI packages, local marketplaces, `file:` / `link:` plugin specs | Machine paths. |
| OpenCode plugins that are local files, git, `github:` or URL specs, and `[spec, options]` entries | Machine paths, and options that may hold secrets. Only npm registry specs are shared. |
| OpenCode servers with a `{env:...}` or `{file:...}` substitution in the command, arguments or URL | OpenCode would put this machine's variables or file contents there. An import refuses them as well. |
| Claude Code servers with a `${...}` substitution in the command, arguments or URL, and servers of another transport (`ws`) | Claude Code would put this machine's variables there. An import refuses them as well. |
| Claude Code marketplaces from a folder or file, from npm, over `ssh` or plain `http`, or pinned to a ref, a folder inside the repository or a sparse checkout | Machine paths, or a source another machine would not add the same way. |
| Claude Code plugins installed only for a project or by the organization, plugins of Claude's own `inline`, `skills-dir` and `synced` entries, and plugins of a marketplace that is left out | Not the user's, local folders, the account's, or not installable from the setup. |
| Sources with credentials | Secrets. |
| Project-scope plugins and servers | The share is user scope only. |
| Auth and credential files | Never opened. |
| Items over the limits | Reason `tooMany`. |
| Plugin enabled/disabled state, app preferences, harness settings | Not part of a setup. |

Host names are not removed: they are part of sources and URLs (`https://mcp.example.com/mcp`, a private git host), so a
setup can reveal which internal hosts you use. Paths and secrets inside free-form MCP arguments are caught by
heuristics, so read the export list before you share a file. Because export runs the same validators as import, an
exported file always imports again with zero invalid items.

## Item statuses

| Status | Meaning | Selectable | Ticked by default |
| --- | --- | --- | --- |
| `new` | Not here yet; it would be added. | yes | yes, except stdio MCP servers |
| `alreadyPresent` | Same marketplace name and source, same plugin id, same PI package source, or an MCP server of the same name and identical definition. | no | |
| `differentVersion` | Installed here in another version (a plugin version, or a PI package with another pin). Informational; never upgraded or downgraded. | no | |
| `differentDefinition` | An MCP server of the same name but another definition, in a file or scope the app writes. Ticking it replaces it: for an http/sse server at the same endpoint only its URL changes and its other settings (headers, on/off) are kept; otherwise (a stdio server, another endpoint) the whole entry is replaced (see [MCP servers](#mcp-servers)). | yes | no, http and stdio alike |
| `conflict` | A marketplace of the same name with a different source. Left untouched. An MCP server is a `conflict` (not selectable) when its name is only in a file the harness reads besides the one the app writes, or in OpenCode's merged config, when such a file exists but cannot be read or parsed, or, for Codex, when the existing entry holds fields the CLI cannot carry over (see [MCP servers](#mcp-servers)). For OpenCode, a name already in its merged config is found only when you import and ends `conflict` then. | no | |
| `blocked` | The harness is not installed here; it has no such kind of item or no user MCP file; or a plugin's marketplace is neither registered here nor in the bundle. | no | |
| `invalid` | The item failed validation, with a typed reason. Never applied. | no | |

A plugin from a marketplace that the same bundle adds is linked to it (`dependsOn`): unticking the marketplace unticks
the plugin, and Rust refuses an apply that selects a plugin without its marketplace unless the marketplace is already
registered here.

## MCP servers

- A **stdio** server runs a program on your computer whenever the harness starts it. It is shown with a highlighted
  notice, starts **unticked**, and the app never starts it itself.
- An **http** or **sse** server does not run a program, but it gives a remote service the right to offer tools to your
  agent, and the tools' output goes into its context. It starts **ticked** like other items; check the URL in the review.
- The entry that is written holds only: for stdio `command` and `args`; for http/sse `type` (`http` or `sse`) and
  `url`. Nothing else is copied.
- A server that declares `envKeys` or `headerKeys` is written, when it is new, with `"enabled": false`. The review lists the names as
  "needs KEY" (when it replaces an http/sse entry of the same endpoint: "Needs KEY. Values already set here are kept."). Nothing is written for them: fill in the values in the harness's own configuration, then enable the
  server. A replace of the same http/sse endpoint never adds `"enabled": false` and never touches the values already there; any other replace is written like a new server, switched off. Only these declared names may be referenced in the arguments (`$NAME`, `${NAME}`).
- The MCP merge is gated by the plan's installed state: a harness that was not installed at review time has no file, and
  its servers end `skipped` (`harnessNotInstalled`).
- Merge rules: existing keys and servers keep their order and values. A name that already exists with the identical
  definition is `alreadyPresent` (arguments are compared exactly, so an entry with a local path argument of its own differs). With
  another definition it is `differentDefinition`, **unticked by default**; ticking it and confirming **replaces** it.
  What a replace keeps depends on whether the existing entry is an http/sse server at the **same endpoint** as the
  shared one (the same transport and the same scheme, host and port), read at review and again at apply time. The
  settings of an entry belong to the server they were set up for, so they are only kept for that case. A stdio server
  never counts: what it connects to hangs on its arguments (`mcp-remote <url>`, a docker image, `--api-url=...`), which
  the shared entry controls.
  - **Same http/sse endpoint:** the replace rewrites only the managed keys in place (`type`, `command`, `args`, `url`;
    OpenCode: `type`, `command` as an array, `url`). Every other key (`env`, `environment`, `headers`, `enabled`,
    `disabled`, `cwd`, `oauth`, and any other) keeps its value and its position, and no `enabled` is added. The review
    shows the step (the file and the managed fields that would be written, never `enabled` or `disabled`) and the row
    says "Replaces in <file>".
  - **Anything else** (any stdio server, another host, scheme or port, any change of transport): the
    whole entry is swapped in place (it keeps its position) for the entry a new add would write, including
    `"enabled": false` (OpenCode 2: `"disabled": true`) when the bundle declares env or header keys. No `env`, `headers`,
    `oauth`, bearer variable, `cwd` or other key of the old entry survives, so nothing set up for another server
    reaches this one. The review shows that full entry, as for a new server.
  The file is refused, and nothing written, if it is invalid
  JSON, a symlink, not a regular file, over 2 MiB, or changed while it was read. A missing file is created with `0600`.
  A refused target file makes its servers not selectable.
- **Automatic conflict resolution.** There is no "changed since review" check. Right before it writes, the app reads the
  target again: if the name now differs it is replaced, if it is now identical the item ends `alreadyPresent`, if it
  is gone it is added as a new server (`done`), and if the name is now in a file the app only reads, or the file cannot be read, it ends `conflict` and nothing is written.
- **Files the harness also reads.** Some harnesses read servers from more files than the one the merge writes (OpenCode's
  other config layers, OMP's `.mcp.json`). The app checks all of them for the name, not only the first match, and never
  writes them. A symlink there is followed (read-only). A file that exists but cannot be read or parsed makes the server
  a `conflict`: the app fails closed, because the harness might merge a server of that name from it and the new entry
  could inherit that server's headers or env.
- **OpenCode's merged config.** For OpenCode the app also asks OpenCode for its merged config (`opencode debug config`,
  run in your home folder with auto-update off and a short timeout, its output never logged or stored). This covers
  layers the app cannot read itself: the organisation config fetched by `opencode auth login`, organisation and managed
  config, `OPENCODE_CONFIG_CONTENT` and a legacy TOML file.
  - **Only when you import.** The check runs after you confirm the import, never while you review a setup. The review
    only reads files, so a new OpenCode server carries a note: its name is checked against OpenCode's merged
    configuration when you import, and that check starts OpenCode once.
  - **What starting OpenCode can change.** The command is not read-only. OpenCode may fetch your organisation's config
    over the network, start a plugin dependency install (`bun add @opencode-ai/plugin`) in its config folders and add a
    `.gitignore` there, create `opencode.jsonc` or add `$schema` to your config, and may migrate a legacy TOML config
    to `config.json` (overwriting an existing one and removing the TOML file). It starts OpenCode fully, so your
    configured plugins load and run, including any plugin this import just installed, and a plugin can change what
    the check sees.
  - **Outcomes.** It runs once per import, after OpenCode's plugins and before any of its servers is written, and only
    when at least one selected OpenCode server is missing from every config file the app reads. A server whose name is
    in the merged config ends `conflict` and is not written. If OpenCode is not found the servers end `skipped`
    (`harnessNotInstalled`), and if the check fails they end `failed`. In both cases no OpenCode MCP server is written
    in that import. Plugins are not affected.
  - **Limits.** If the app runs with another `XDG_DATA_HOME` than your shell (for example started from a desktop
    launcher), OpenCode may not find the login that fetches the organisation config, so names from that layer can be
    missed. OpenCode also names a server's tools by a sanitized server name, so servers such as `foo.bar` and `foo_bar`
    produce the same tool names: one server's tools can shadow the other's. They are still separate entries and do not
    share headers or env.
- **Claude Code.** The user scope of `~/.claude.json` (or `$CLAUDE_CONFIG_DIR/.claude.json`) is read for the name,
  and each new server is added with `claude mcp add-json --scope user <name> <json>`, the command line the review
  shows. The JSON holds `type` and the `command` and `args` or the `url`, nothing else. Claude Code has no `enabled`
  field, so a server that declares `envKeys` or `headerKeys` is added switched on and without values: a stdio server
  sees Claude Code's environment, and the values can be added in Claude Code afterwards. Each run needs the
  `claude` binary, like the CLI items. `add-json` refuses a name the user scope has, so a **replace** is two runs, shown as two steps:
  `claude mcp remove --scope user <name>`, then `claude mcp add-json --scope user <name> <json>`. At apply time the
  app reads the existing entry from `~/.claude.json`. For the same endpoint it builds the new JSON from it with only the
  managed keys swapped, so `env`, `headers` and `oauth` are kept; for another endpoint the JSON is the fresh entry and
  nothing of the old one is kept. If the add fails, the old entry is added back. Because kept `env` and `headers` values
  go onto that command line, the process list on this machine shows them for the duration of the run; the result in the
  app shows the command without values, and the values are never stored in the review or sent to the window. The CLI's
  output is never logged for a replace.
- **Codex.** Servers are added with `codex mcp add <name> …` (`--url` for http, `-- <command> <args>` for stdio), never by writing `config.toml`.
  A **replace** is `codex mcp remove <name>` followed by `codex mcp add`. For the same http endpoint the add carries
  the existing `bearer_token_env_var` through its flag; a stdio server or another endpoint is a plain `codex mcp add`
  of the shared server, and nothing of the old entry (its `env` included) is carried over. If the add fails the old entry
  is added back, with its `env` as `--env KEY=VALUE` on that command line for one run (visible in the local process
  list, never stored or shown). So an existing entry that holds anything `codex mcp add` cannot write back (a timeout,
  a tool list, another auth setting, `enabled`) stays a `conflict` and is not selectable, whatever it points to.
- **OpenCode config files with comments.** Such a file is never rewritten: a new server there ends `skipped` (`manualEdit`)
  and the import's result shows the entry to paste. A replace of the same http/sse endpoint there ends `skipped` (`manualReplace`) and shows the
  managed fields only (`type`, `command`/`url`): replace those in the server's entry yourself and keep the others. A
  replace of another endpoint ends `skipped` (`manualEdit`) with the whole entry: replace the server's entry with it.

## Applying

Order: harnesses one after another; within a harness the CLI items in plan order (marketplaces, plugins, packages),
then the MCP servers, written with one merge per file (Claude Code and Codex: one CLI run, or a remove and an add for a replace, per server). Only one apply runs at a time.

- Rust refuses the whole request (nothing runs) without `confirmed`, while another import is running, with an unknown or
  stale plan id, an empty selection, an id that is not a selectable item of that plan, or a plugin whose marketplace
  was unticked.
- **The plan is single-use.** An attempt without `confirmed`, while another apply is running, or with another plan id
  leaves the stored review as it was. Once the plan id matches, the review is taken: whatever happens next (it applies,
  a harness lookup fails, or Rust refuses the selection as empty, unknown or missing a marketplace) it is used up and
  you review the setup again.
- **Binaries.** Before applying, the app looks up the binary of each harness with installed, selectable items. If that
  lookup fails, the whole import fails with the real error and nothing runs. A harness that is simply not installed is no failure: its
  CLI items end `skipped` (`harnessNotInstalled`).
- Each item is re-checked against the machine right before its step. Applying the same setup twice therefore runs
  nothing the second time: items end `alreadyPresent`.
- **Partial failure, no rollback.** A failed marketplace skips the plugins that depend on it ("dependency failed");
  every other item still runs. What succeeded stays. The result lists each item's outcome (`done`, `replaced`,
  `alreadyPresent`, `conflict`, `skipped`, `failed`) with the CLI's redacted one-line summary and the command line that
  ran. `replaced` counts as a success like `done`. For a Claude Code or Codex replace that carries values on the command line, the command shown has no values and the summary is a fixed text.

## Forward compatibility

- `format` must match exactly. A `version` above 1 is refused with "made by a newer version"; update the app.
- Unknown fields are ignored everywhere (the schema never denies unknown fields).
- An unknown harness id, or an unknown item kind in a known harness, is counted and listed in the review footer, never
  executed. The key is shown only if it matches `^[A-Za-z0-9_-]{1,32}$` (otherwise "unknown"); only the item count is
  shown, never the contents.
- Structural errors (prefix, base64, zlib, size, JSON root, `format`, `version`, section or item counts) reject the
  whole input. Problems inside one item only mark that item `invalid`.
- A repeated item id in one bundle keeps the first.

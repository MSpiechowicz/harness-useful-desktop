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

An import only **adds**. These are the only actions it can produce, and only in the user scope:

| Item | What runs |
| --- | --- |
| Marketplace (OMP) | `omp plugin marketplace add <source>` |
| Plugin (OMP) | `omp plugin install <id> --scope user` |
| Package (PI) | `pi install <source>` (global, no `--local`) |
| MCP server | One atomic merge of the entry into the harness's user `mcp.json` |

It never uninstalls, removes, upgrades, downgrades, overwrites, or touches project scope, app preferences or harness
settings. Something already present is left alone. Versions in a bundle are informational: the latest version is
installed (a PI source keeps any pin already written in the source string). Plugin enabled/disabled state is not
copied.

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
| `harnesses` | object | One section per harness id (`omp`, `pi`). |

A harness section holds up to four arrays; which ones a harness accepts depends on what it supports (OMP:
marketplaces, plugins, MCP servers; PI: packages, MCP servers).

| Array | Field | Type | Meaning |
| --- | --- | --- | --- |
| `marketplaces` | `name` | string | `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`. |
| | `source` | string | A GitHub `owner/repo` or an `https://` URL on a host name. |
| `plugins` | `id` | string | `name@marketplace` or an npm package name. |
| | `marketplace` | string, optional | Must match the `@marketplace` part of `id`. |
| | `version` | string, optional | Informational. |
| `packages` | `source` | string | `npm:…`, `git:…` or `https://…`, with any pin it had (see [Validation](#validation-of-every-item)). |
| | `version` | string, optional | Informational. |
| `mcpServers` | `name` | string | `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, never `harness-dashboard` or `harness-memory` (reserved). |
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
  `@marketplace` part.
- **MCP server names:** the pattern above. `harness-dashboard` and `harness-memory` (any case) are reserved: Recommended setup installs and
  registers the first, and the app installs and registers the second (project memory), so neither is ever exported and an
  imported one is refused.
- **MCP commands:** a bare program name, never a path and never a shell interpreter (`sh`, `bash`, `zsh`, `dash`,
  `fish`, `cmd`, `pwsh`, `powershell`, with or without a directory or `.exe`). Never `harness-dashboard` or
  `harness-memory` either (any case, with or without `.exe`): together with the reserved names, an import can neither
  add the app's two entries nor shadow them with a server of another name that runs the same program.
- **Environment references.** A harness expands `$NAME` and `${NAME}` from its own environment, so a reference in a
  received setup could hand a variable of this machine to the setup's author. A URL may hold none. An argument may only
  reference a name the same item lists in `envKeys`, as a plain `$NAME` or `${NAME}` (and then the server is written
  disabled); an undeclared name and any other `${…}` form (a default such as `${NAME:-x}`, an unclosed brace) are
  refused. A `$` that does not start a name (`5$`, `$1`) is not a reference.
- **MCP URLs:** `http://` or `https://` with a host, no whitespace, no user info, no `${…}` or `$NAME` (even a declared
  one), no secret query value (`?api_key=…`).
- **Machine paths.** On export, an MCP server whose arguments or URL hold a path of this machine is left out whole: an
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
| MCP servers whose command, arguments or URL hold a secret, credentials or a machine path | The whole server is left out. Heuristics (see [Validation](#validation-of-every-item)), not a proof. |
| MCP servers with an undeclared `${VAR}` / `$NAME` reference, a shell command, a non-bare command, or a `harness-dashboard` / `harness-memory` command under another name | Would hand an environment variable to the setup's author, run a shell, or shadow an app entry on the other machine. |
| The `harness-dashboard` server (any letter case) | Recommended setup installs it. Skipped without a row in the list. |
| The `harness-memory` server (any letter case) and the project memory database | The app installs and registers the server itself, and the notes are local to this machine. Skipped without a row in the list; an import refuses that name. |
| Local or unknown PI packages, local marketplaces, `file:` / `link:` plugin specs | Machine paths. |
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
| `conflict` | An MCP server or marketplace of the same name with a different definition. Left untouched. | no | |
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
- A server that declares `envKeys` or `headerKeys` is written with `"enabled": false`. The review lists the names as
  "needs KEY". Nothing is written for them: fill in the values in the harness's own configuration, then enable the
  server. Only these declared names may be referenced in the arguments (`$NAME`, `${NAME}`).
- The MCP merge is gated by the plan's installed state: a harness that was not installed at review time has no file, and
  its servers end `skipped` (`harnessNotInstalled`).
- Merge rules: existing keys and servers keep their order and values; a name that already exists is never overwritten
  (identical is `alreadyPresent`, different is `conflict`). The file is refused, and nothing written, if it is invalid
  JSON, a symlink, not a regular file, over 2 MiB, or changed while it was read. A missing file is created with `0600`.

## Applying

Order: harnesses one after another; within a harness the CLI items in plan order (marketplaces, plugins, packages),
then the MCP servers, written with one merge per file. Only one apply runs at a time.

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
  every other item still runs. What succeeded stays. The result lists each item's outcome (`done`, `alreadyPresent`,
  `conflict`, `skipped`, `failed`) with the CLI's redacted one-line summary and the command line that ran.

## Forward compatibility

- `format` must match exactly. A `version` above 1 is refused with "made by a newer version"; update the app.
- Unknown fields are ignored everywhere (the schema never denies unknown fields).
- An unknown harness id, or an unknown item kind in a known harness, is counted and listed in the review footer, never
  executed. The key is shown only if it matches `^[A-Za-z0-9_-]{1,32}$` (otherwise "unknown"); only the item count is
  shown, never the contents.
- Structural errors (prefix, base64, zlib, size, JSON root, `format`, `version`, section or item counts) reject the
  whole input. Problems inside one item only mark that item `invalid`.
- A repeated item id in one bundle keeps the first.

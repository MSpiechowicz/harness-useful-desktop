//! The app's own MCP server (`harness-desktop mcp`, see [`crate::hooks_mcp`]) in a harness's user
//! MCP file, so agents can manage hooks: [`register_desktop_mcp`] at launch (release builds) and
//! on the user's request, [`desktop_mcp_state`] for the setup step.
//!
//! - Found: a server named [`DESKTOP_MCP_SERVER`], or any server running `harness-desktop mcp`
//!   (any directory, `.exe` or not), in the target file or one of the `also_read` files (on
//!   OpenCode 2 under `mcp.servers` or the legacy `mcp`), switched off by the target file's
//!   `disabledServers` or by the server itself (as in [`super::mcp_register`]).
//! - Owned: an entry in the target file (never an `also_read` one) named exactly
//!   [`DESKTOP_MCP_SERVER`] whose only argument is `mcp` and whose program is the app: the current
//!   one, the one the app last wrote into that file (the record, see [`super::desktop_record`]), or
//!   any `harness-desktop[.exe]`. Only an owned entry is changed: its program follows the app
//!   (an AppImage moved, an update installed elsewhere), every other field and the key order kept.
//! - Added: the file's [`Shape::entry`] for the app's absolute path, only when nothing is found and
//!   the app has not registered the server in that file before (a user who removed it keeps it
//!   removed).
//!
//! Writes follow [`super::mcp_merge`]'s rules: the same refusals, a JSONC file with comments never
//! written, a new file `0600`, an atomic write keeping permissions, one re-read when the file
//! changed meanwhile. File contents are never logged or returned.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::mcp_merge::{Blocked, Change, Shape, Target, change_file, load, server_mut};
use super::mcp_register::{FoundServer, add_server, find_server, server_program};
use super::setup::McpFile;
use crate::env::Env;
use crate::model::{McpRefusal, McpState};
use crate::pi_family::tilde;

/// The server name the app registers (also the MCP server's `serverInfo.name`).
pub const DESKTOP_MCP_SERVER: &str = crate::hooks_mcp::protocol::SERVER_NAME;

/// The app's executable name (the `[[bin]]` of `src-tauri`).
pub const DESKTOP_BINARY: &str = "harness-desktop";

/// What [`register_desktop_mcp`] did with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopRegistration {
    /// The entry was written.
    Added,
    /// The owned entry ran another program; its `command` now names the app.
    Updated,
    /// The server is there and on.
    Present,
    /// The server is there but switched off; left alone.
    Disabled,
    /// Nothing is there, but the app registered it before: the user removed it, so it stays out.
    SkippedRemoved,
    /// The file has comments (JSONC) a rewrite would lose; nothing was written.
    ManualEdit,
    /// The file cannot be changed safely; nothing was written.
    Refused(McpRefusal),
}

impl DesktopRegistration {
    /// Whether the server is in the file after this registration (enabled or not).
    pub fn is_registered(self) -> bool {
        matches!(self, Self::Added | Self::Updated | Self::Present | Self::Disabled)
    }

    /// Whether the app wrote its program into the file.
    pub fn wrote(self) -> bool {
        matches!(self, Self::Added | Self::Updated)
    }
}

/// The program harnesses start for the app: the AppImage file while the app runs from one, else
/// `current_exe`. It runs from one when the AppImage runtime set both variables: `appdir` (its
/// mount, absolute) holds `current_exe` (the executable is inside a mount that changes on every
/// launch) and `appimage` is absolute. `APPIMAGE` alone could be anything (inherited from another
/// AppImage, or set by hand), like in `process::appimage`.
pub fn app_command(current_exe: &Path, appimage: Option<&OsStr>, appdir: Option<&OsStr>) -> PathBuf {
    let inside_mount = appdir
        .map(Path::new)
        .is_some_and(|appdir| appdir.is_absolute() && current_exe.starts_with(appdir));

    match appimage
        .map(Path::new)
        .filter(|path| inside_mount && path.is_absolute())
    {
        Some(appimage) => appimage.to_path_buf(),
        None => current_exe.to_path_buf(),
    }
}

/// [`app_command`] of this process; `None` when its executable is unknown.
pub fn current_app_command() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;

    Some(app_command(
        &exe,
        std::env::var_os("APPIMAGE").as_deref(),
        std::env::var_os("APPDIR").as_deref(),
    ))
}

/// Registers the app's server in `file` as `command mcp` (see the module docs). `recorded` tells
/// that the app registered it in that file before, `previous` the program it wrote there last.
pub fn register_desktop_mcp(
    file: &McpFile,
    command: &Path,
    recorded: bool,
    previous: Option<&str>,
) -> DesktopRegistration {
    let command = command.to_string_lossy();
    let changed = change_file(&file.path, file.shape, |target| {
        desktop_change(file, target, &command, recorded, previous)
    });

    desktop_outcome(changed)
}

/// What registering the app's server as `command mcp` does with `target` (see the module docs).
pub(crate) fn desktop_change(
    file: &McpFile,
    target: &Target,
    command: &str,
    recorded: bool,
    previous: Option<&str>,
) -> Change<DesktopRegistration> {
    let Some(server) = find_server(file, target, DESKTOP_MCP_SERVER, DESKTOP_BINARY) else {
        if recorded {
            return Change::Keep(DesktopRegistration::SkippedRemoved);
        }

        let mut document = target.document();
        add_server(&mut document, file.shape, DESKTOP_MCP_SERVER, file.shape.entry(command));
        return Change::Write(document, DesktopRegistration::Added);
    };

    let program = server_program(&server.definition);
    let follows = is_owned(&file.path, &server, command, previous) && program != Some(command);

    if follows {
        tracing::info!(
            file = %file.path.display(),
            from = program.unwrap_or_default(),
            to = %command,
            "the app's MCP entry now runs this app"
        );

        let mut document = target.document();
        repoint(&mut document, file.shape, command);
        return Change::Write(document, DesktopRegistration::Updated);
    }

    Change::Keep(if server.disabled {
        DesktopRegistration::Disabled
    } else {
        DesktopRegistration::Present
    })
}

/// The registration a desktop change ends in.
pub(crate) fn desktop_outcome(changed: Result<DesktopRegistration, Blocked>) -> DesktopRegistration {
    changed.unwrap_or_else(|blocked| match blocked {
        Blocked::Refused(reason) => DesktopRegistration::Refused(reason),
        Blocked::ManualEdit => DesktopRegistration::ManualEdit,
    })
}

/// The app's server in `file`, read-only, for the setup step. A manual edit names `command` (the
/// app's program, else its executable name).
pub fn desktop_mcp_state(env: &Env, file: &McpFile, command: Option<&Path>) -> McpState {
    let at = |path: &Path| tilde(env, path);

    let target = match load(&file.path, file.shape) {
        Ok(target) => target,
        Err(reason) => {
            return McpState::Refused {
                file: at(&file.path),
                reason,
            };
        }
    };

    if let Some(server) = find_server(file, &target, DESKTOP_MCP_SERVER, DESKTOP_BINARY) {
        let (file, name) = (at(&server.file), server.name);

        if server.disabled {
            return McpState::Disabled { file, name };
        }

        return McpState::Configured { file, name };
    }

    if target.rewritable() {
        return McpState::Missing {
            file: at(&file.path),
            exists: matches!(target, Target::Present { .. }),
        };
    }

    let program = command.map_or_else(|| DESKTOP_BINARY.to_owned(), |path| path.to_string_lossy().into_owned());
    let entry = serde_json::to_string_pretty(&file.shape.entry(&program)).unwrap_or_default();

    McpState::ManualEdit {
        file: at(&file.path),
        snippet: format!("{}: {entry}", Value::String(DESKTOP_MCP_SERVER.to_owned())),
    }
}

/// Whether the app may change `server` (see the module docs).
fn is_owned(path: &Path, server: &FoundServer, command: &str, previous: Option<&str>) -> bool {
    if server.file != path || server.name != DESKTOP_MCP_SERVER || !only_mcp_argument(&server.definition) {
        return false;
    }

    let Some(program) = server_program(&server.definition) else {
        return false;
    };

    let file_name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let named_like_the_app = file_name.strip_suffix(".exe").unwrap_or(&file_name) == DESKTOP_BINARY;

    program == command || Some(program) == previous || named_like_the_app
}

/// True when the entry's only argument is `mcp`: `"args": ["mcp"]`, or a `command` array of the
/// program and `mcp`.
fn only_mcp_argument(server: &Value) -> bool {
    let mcp = [Value::String("mcp".to_owned())];

    match server.get("command") {
        Some(Value::String(_)) => server
            .get("args")
            .and_then(Value::as_array)
            .is_some_and(|args| args[..] == mcp),
        Some(Value::Array(parts)) => parts.get(1..).is_some_and(|args| args == mcp),
        _ => false,
    }
}

/// Points the owned entry at `program` in place: its `command`, or the first item of its `command`
/// array.
fn repoint(document: &mut Map<String, Value>, shape: Shape, program: &str) {
    let slot = server_mut(document, shape, DESKTOP_MCP_SERVER).and_then(|entry| entry.get_mut("command"));

    match slot {
        Some(Value::Array(parts)) if !parts.is_empty() => parts[0] = Value::String(program.to_owned()),
        Some(command) => *command = Value::String(program.to_owned()),
        None => {}
    }
}

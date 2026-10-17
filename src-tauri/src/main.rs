// Release builds on Windows have no console window. `harness-desktop mcp` still serves on the
// stdio pipes its parent (the harness) passes: a GUI-subsystem process inherits redirected
// standard handles, it only gets no console of its own (none is allocated).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

use harness_business::hooks_mcp::{MCP_ARG, run_stdio};

fn main() -> ExitCode {
    // `harness-desktop mcp`: the hooks MCP server for agents on stdin and stdout, no window and no
    // Tauri runtime (so no single-instance check either), until stdin closes.
    if std::env::args_os().nth(1).is_some_and(|arg| arg == MCP_ARG) {
        return run_stdio();
    }

    harness_desktop_lib::run()
}

# harness-useful-desktop
Desktop GUI for AI harness agents

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

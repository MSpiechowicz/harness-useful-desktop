import type { AppInfo } from './bindings/AppInfo';
import type { AppPrefs } from './bindings/AppPrefs';

/** Phantom entry: carries the argument and result types of one command, no runtime data. */
export interface CommandSpec<Args, Result> {
  readonly args: Args;
  readonly result: Result;
}

function cmd<Args, Result>(): CommandSpec<Args, Result> {
  return {} as CommandSpec<Args, Result>;
}

/**
 * The single typed command map. Keys are the snake_case names of the Rust
 * `#[tauri::command]` functions; `scripts/check-ipc-contract.ts` compares them 1:1.
 */
export const commands = {
  app_info: cmd<undefined, AppInfo>(),
  prefs_get: cmd<undefined, AppPrefs>(),
  prefs_set: cmd<{ prefs: AppPrefs }, AppPrefs>(),
};

export type CommandName = keyof typeof commands;
export type CommandArgs<K extends CommandName> = (typeof commands)[K]['args'];
export type CommandResult<K extends CommandName> = (typeof commands)[K]['result'];

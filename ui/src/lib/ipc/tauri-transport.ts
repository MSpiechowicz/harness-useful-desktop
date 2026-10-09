import { invoke } from '@tauri-apps/api/core';

import type { CommandName } from './commands';
import { IpcFailure, type Transport } from './transport';

/** Calls the Rust `#[tauri::command]` of the same name; rejected payloads become `IpcFailure`. */
export function createTauriTransport(): Transport {
  return {
    async invoke<K extends CommandName>(name: K, args: Parameters<Transport['invoke']>[1]) {
      try {
        return await invoke(name, args as Record<string, unknown> | undefined);
      } catch (error) {
        throw IpcFailure.from(error);
      }
    },
  };
}

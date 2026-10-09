import { Channel, invoke } from '@tauri-apps/api/core';

import type { CommandName } from './commands';
import { IpcFailure, type EventSink, type Transport } from './transport';

function toChannel<T>(sink: EventSink<T>): Channel<T> {
  const channel = new Channel<T>();
  channel.onmessage = sink;

  return channel;
}

/** Event sinks (functions) become Tauri channels; every other argument passes through untouched. */
function withChannels(args: Record<string, unknown> | undefined): Record<string, unknown> | undefined {
  if (!args) {
    return args;
  }

  const entries = Object.entries(args).map(([name, value]) => [name, typeof value === 'function' ? toChannel(value as EventSink<unknown>) : value]);

  return Object.fromEntries(entries);
}

/** Calls the Rust `#[tauri::command]` of the same name; rejected payloads become `IpcFailure`. */
export function createTauriTransport(): Transport {
  return {
    async invoke<K extends CommandName>(name: K, args: Parameters<Transport['invoke']>[1]) {
      try {
        return await invoke(name, withChannels(args as Record<string, unknown> | undefined));
      } catch (error) {
        throw IpcFailure.from(error);
      }
    },
  };
}

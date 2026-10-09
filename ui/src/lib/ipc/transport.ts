import type { IpcError } from './bindings/IpcError';
import type { IpcErrorCode } from './bindings/IpcErrorCode';
import type { CommandArgs, CommandName, CommandResult } from './commands';

/**
 * Receives the events of a streaming command. Passed as a command argument; the Tauri transport
 * turns it into a `Channel`, the mock transport calls it directly.
 */
export type EventSink<T> = (event: T) => void;

/** The seam between the UI and the backend; Tauri, mock and "unavailable" implement it. */
export interface Transport {
  invoke<K extends CommandName>(name: K, args: CommandArgs<K>): Promise<CommandResult<K>>;
}

const FALLBACK_CODE: IpcErrorCode = 'internal';

function isIpcError(value: unknown): value is IpcError {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Record<string, unknown>;

  return typeof candidate.code === 'string' && typeof candidate.message === 'string';
}

/** Typed error thrown by every transport; wraps the backend's `IpcError { code, message }`. */
export class IpcFailure extends Error {
  readonly code: IpcErrorCode;

  constructor(error: IpcError) {
    super(error.message);
    this.name = 'IpcFailure';
    this.code = error.code;
  }

  /** Normalizes anything a transport can reject with (IpcError payload, string, Error). */
  static from(value: unknown): IpcFailure {
    if (value instanceof IpcFailure) {
      return value;
    }
    if (isIpcError(value)) {
      return new IpcFailure(value);
    }
    if (value instanceof Error) {
      return new IpcFailure({ code: FALLBACK_CODE, message: value.message });
    }

    return new IpcFailure({ code: FALLBACK_CODE, message: String(value) });
  }
}

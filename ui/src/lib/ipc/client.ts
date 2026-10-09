import { createTauriTransport } from './tauri-transport';
import { IpcFailure, type Transport } from './transport';

/** Used when neither the Tauri runtime nor a mock build is present (e.g. plain `vite` in a browser). */
function createUnavailableTransport(): Transport {
  const failure = new IpcFailure({
    code: 'internal',
    message: 'The desktop runtime is not available. Start the app with `bun run dev`, or `bun run dev:mock` for a browser preview.',
  });

  return {
    invoke: () => Promise.reject(failure),
  };
}

/**
 * Picks the transport for this environment. The mock branch is guarded by the
 * literal build-time constant so production bundles contain no mock code.
 */
export async function selectTransport(hasTauri: boolean = '__TAURI_INTERNALS__' in window): Promise<Transport> {
  if (hasTauri) {
    return createTauriTransport();
  }

  if (import.meta.env.VITE_TRANSPORT === 'mock') {
    const { createMockTransport } = await import('./mock/transport');

    return createMockTransport();
  }

  return createUnavailableTransport();
}

let selected: Promise<Transport> | undefined;

function getTransport(): Promise<Transport> {
  selected ??= selectTransport();

  return selected;
}

/** The transport every store and feature talks to; resolves the real one lazily on first use. */
export const ipc: Transport = {
  async invoke(name, args) {
    const transport = await getTransport();

    return transport.invoke(name, args);
  },
};

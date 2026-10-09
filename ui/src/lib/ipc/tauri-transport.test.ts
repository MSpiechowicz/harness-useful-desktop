import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

type TauriModule = typeof import('./tauri-transport');
type TransportModule = typeof import('./transport');

let createTauriTransport: TauriModule['createTauriTransport'];
let IpcFailure: TransportModule['IpcFailure'];

describe('createTauriTransport', () => {
  // The test setup already loaded the real module graph, so reload it with the mock in place.
  beforeEach(async () => {
    vi.resetModules();
    ({ createTauriTransport } = await import('./tauri-transport'));
    ({ IpcFailure } = await import('./transport'));
  });

  it('returns the command result unchanged', async () => {
    invoke.mockResolvedValueOnce({ name: 'Harness Desktop' });

    await expect(createTauriTransport().invoke('app_info', undefined)).resolves.toEqual({ name: 'Harness Desktop' });
  });

  it('turns a rejected IpcError payload into a typed IpcFailure', async () => {
    invoke.mockRejectedValueOnce({ code: 'forbidden', message: 'path is deny-listed' });

    const failure = await createTauriTransport()
      .invoke('app_info', undefined)
      .catch((error: unknown) => error);

    expect(failure).toBeInstanceOf(IpcFailure);
    expect(failure).toMatchObject({ code: 'forbidden', message: 'path is deny-listed' });
  });

  it('wraps unexpected rejections (strings, errors) as internal failures', async () => {
    invoke.mockRejectedValueOnce('command app_info not found');

    await expect(createTauriTransport().invoke('app_info', undefined)).rejects.toMatchObject({
      code: 'internal',
      message: 'command app_info not found',
    });
  });
});

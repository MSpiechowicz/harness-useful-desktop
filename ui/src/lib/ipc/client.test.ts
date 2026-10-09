import { afterEach, describe, expect, it, vi } from 'vitest';

import { selectTransport } from './client';
import { IpcFailure } from './transport';

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('selectTransport', () => {
  it('uses the mock transport in a mock build outside Tauri', async () => {
    vi.stubEnv('VITE_TRANSPORT', 'mock');

    const transport = await selectTransport(false);
    const info = await transport.invoke('app_info', undefined);

    expect(info.name).toBe('Harness Desktop');
  });

  it('serves persistent in-memory preferences from the mock', async () => {
    vi.stubEnv('VITE_TRANSPORT', 'mock');
    const transport = await selectTransport(false);
    const prefs = await transport.invoke('prefs_get', undefined);

    await transport.invoke('prefs_set', { prefs: { ...prefs, theme: 'light' } });

    expect((await transport.invoke('prefs_get', undefined)).theme).toBe('light');
  });

  it('rejects every call with a clear error when no backend is available', async () => {
    vi.stubEnv('VITE_TRANSPORT', '');

    const transport = await selectTransport(false);
    const attempt = transport.invoke('app_info', undefined);

    await expect(attempt).rejects.toBeInstanceOf(IpcFailure);
    await expect(attempt).rejects.toThrow(/desktop runtime is not available/);
  });
});

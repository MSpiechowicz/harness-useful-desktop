import type { AppInfo } from '$lib/ipc/bindings/AppInfo';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

export type AppStatus = 'loading' | 'ready' | 'unavailable';

/** Application facts and backend connectivity. */
export class AppStore {
  info = $state<AppInfo | null>(null);
  status = $state<AppStatus>('loading');
  failure = $state<IpcFailure | null>(null);

  constructor(private readonly transport: Transport) {}

  async load(): Promise<void> {
    try {
      this.info = await this.transport.invoke('app_info', undefined);
      this.status = 'ready';
      this.failure = null;
    } catch (error) {
      this.failure = IpcFailure.from(error);
      this.status = 'unavailable';
    }
  }
}

export const app = new AppStore(ipc);

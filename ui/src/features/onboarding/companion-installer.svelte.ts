import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import type { InstallLogLine } from '$lib/ipc/bindings/InstallLogLine';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import type { Companion } from './setup-model';

/** The installer's output is kept to its last lines; the backend already caps what it streams. */
const LOG_LINES = 400;

/** How an installer run ended; `failure` is empty when it succeeded. */
export interface InstallOutcome {
  ok: boolean;
  failure: string;
  /** The binary's version after a successful run. */
  version: string | null;
}

/**
 * One companion's installer run (the dashboard's or the memory server's): the output it streams, and how it ended.
 * The backend only runs it once told the user confirmed, which is the caller's business.
 */
export class CompanionInstaller {
  log = $state.raw<InstallLogLine[]>([]);
  /** The installer's outcome of the last run, for the retry hint. */
  result = $state.raw<DashboardInstallResult | null>(null);

  constructor(
    private readonly companion: Companion,
    private readonly transport: Transport,
  ) {}

  reset(): void {
    this.log = [];
    this.result = null;
  }

  async run(): Promise<InstallOutcome> {
    this.reset();

    try {
      const onLog = (line: InstallLogLine) => (this.log = [...this.log, line].slice(-LOG_LINES));
      const args = { confirmed: true, onLog };
      const result = await (this.companion === 'dashboard' ? this.transport.invoke('setup_install_dashboard', args) : this.transport.invoke('setup_install_memory', args));
      this.result = result;

      return { ok: result.ok, failure: result.ok ? '' : (result.summary ?? ''), version: result.version };
    } catch (error) {
      return { ok: false, failure: IpcFailure.from(error).message, version: null };
    }
  }
}

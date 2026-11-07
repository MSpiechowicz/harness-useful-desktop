import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import type { InstallLogLine } from '$lib/ipc/bindings/InstallLogLine';
import type { CommandName } from '$lib/ipc/commands';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import type { Installer } from './setup-model';

/** The installer's output is kept to its last lines; the backend already caps what it streams. */
const LOG_LINES = 400;

/** How an installer run ended; `failure` is empty when it succeeded. */
export interface InstallOutcome {
  ok: boolean;
  failure: string;
  /** The binary's version after a successful run. */
  version: string | null;
  /** Another run of this installer holds it: this one did nothing. */
  alreadyRunning: boolean;
}

/** The backend's refusal while another run of the same installer is going. */
export function isAlreadyRunning(failure: IpcFailure): boolean {
  return failure.code === 'invalidInput' && /already being installed/i.test(failure.message);
}

/** The command that runs each installer; the log lines it streams come back on `onLog`. */
const COMMANDS = {
  dashboard: 'setup_install_dashboard',
  memory: 'setup_install_memory',
  knowledge: 'setup_install_knowledge',
  code: 'setup_install_code',
  opencodeSkills: 'setup_install_opencode_skills',
  codexSkills: 'setup_install_codex_skills',
} as const satisfies Record<Installer, CommandName>;

/**
 * One installer run (the dashboard's, the memory server's, the knowledge server's, the code server's or the Harness Skills' for OpenCode or Codex): the output it streams, and how it ended.
 * The backend only runs it once told the user confirmed, which is the caller's business.
 */
export class CompanionInstaller {
  log = $state.raw<InstallLogLine[]>([]);
  /** The installer's outcome of the last run, for the retry hint. */
  result = $state.raw<DashboardInstallResult | null>(null);

  constructor(
    private readonly installer: Installer,
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
      const result = await this.transport.invoke(COMMANDS[this.installer], args);
      this.result = result;

      return { ok: result.ok, failure: result.ok ? '' : (result.summary ?? ''), version: result.version, alreadyRunning: false };
    } catch (error) {
      const failure = IpcFailure.from(error);

      return { ok: false, failure: failure.message, version: null, alreadyRunning: isAlreadyRunning(failure) };
    }
  }
}

import { t } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { InstallLogLine } from '$lib/ipc/bindings/InstallLogLine';
import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';

import { failureLine } from '../extensions/actions/action-model';
import { reloadChatsAfterPluginChange } from '../extensions/actions/action-runner.svelte';
import { onboarding } from './onboarding-state.svelte';
import { commandOf, refusalText, type ItemPhase, type SetupKey } from './setup-model';

export type SetupLoad = 'idle' | 'loading' | 'ready' | 'error';

/** The installer's output is kept to its last lines; the backend already caps what it streams. */
const LOG_LINES = 400;

/** One line of what the confirmation lists. */
export interface PlannedCommand {
  /** Whose command it is: the dashboard or a harness. */
  owner: 'dashboard' | HarnessId;
  command: string;
}

/** A file the run will touch, and whether it is created. */
export interface PlannedFile {
  file: string;
  creates: boolean;
}

/** Everything a run would do for the current selection, shown before anything runs. */
export interface Plan {
  commands: PlannedCommand[];
  files: PlannedFile[];
  /** What the dashboard installer changes besides the binary. */
  sideEffects: string[];
}

interface Hooks {
  /** Told when a run starts and ends, so the flow cannot be left meanwhile. */
  busy: (busy: boolean) => void;
  /** After plugin changes, so open chats pick them up. */
  chatsChanged: (harness: HarnessId) => Promise<void>;
  /** After a run, so the Ecosystem screen shows the new state. */
  companionsChanged: () => Promise<void>;
}

/**
 * The "Recommended setup" step. Opening it only reads the machine; nothing runs until the user has seen the
 * list of commands and files and confirmed it. A run goes in a fixed order: the dashboard, then the skills of each
 * harness, then the dashboard's entry in each harness's MCP file.
 */
export class SetupController {
  data = $state.raw<RecommendedSetup | null>(null);
  load = $state<SetupLoad>('idle');
  error = $state<string | null>(null);
  running = $state(false);
  /** The confirmation dialog is open. */
  confirming = $state(false);

  /** Items the user switched off; everything that can be set up starts on. */
  private off = $state<Record<string, boolean>>({});
  private phases = $state<Record<string, ItemPhase>>({});
  private failures = $state<Record<string, string>>({});

  log = $state.raw<InstallLogLine[]>([]);
  /** The installer's outcome of the last run, for the retry hint. */
  installResult = $state.raw<DashboardInstallResult | null>(null);

  private token = 0;

  constructor(
    private readonly transport: Transport,
    private readonly hooks: Hooks,
  ) {}

  // ── Reading ──────────────────────────────────────────────────────────────

  /** Reads the machine's state and forgets any earlier selection and run. Read-only. */
  async open(): Promise<void> {
    if (this.running) {
      return;
    }

    this.token += 1;
    const mine = this.token;

    this.load = 'loading';
    this.error = null;
    this.off = {};
    this.phases = {};
    this.failures = {};
    this.log = [];
    this.installResult = null;

    try {
      const data = await this.transport.invoke('setup_status', undefined);

      if (mine === this.token) {
        this.data = data;
        this.load = 'ready';
      }
    } catch (error) {
      if (mine === this.token) {
        this.data = null;
        this.error = IpcFailure.from(error).message;
        this.load = 'error';
      }
    }
  }

  /** Reads the machine again without touching the selection, phases or log (after a run). */
  private async refresh(): Promise<void> {
    try {
      this.data = await this.transport.invoke('setup_status', undefined);
    } catch {
      // The rows keep what the run itself reported.
    }
  }

  harness(id: HarnessId): HarnessSetup | undefined {
    return this.data?.harnesses.find((entry) => entry.harness === id);
  }

  phase(key: SetupKey): ItemPhase {
    return this.phases[key] ?? 'idle';
  }

  /** The phase a chip shows: a finished item reverts to the state read from the machine once the run is over. */
  shown(key: SetupKey): ItemPhase {
    const phase = this.phase(key);

    return phase === 'done' && !this.running ? 'idle' : phase;
  }

  failure(key: SetupKey): string | null {
    return this.failures[key] ?? null;
  }

  // ── Selection ────────────────────────────────────────────────────────────

  /** Whether the dashboard installer can be chosen: it is missing and runs on this system. */
  get dashboardSelectable(): boolean {
    return !!this.data && !this.data.dashboard.installed && this.data.dashboard.supportedOnThisOs;
  }

  /** The dashboard will be installed, or already is: the precondition of every MCP entry. */
  private get dashboardAvailable(): boolean {
    return !!this.data && (this.data.dashboard.installed || this.isSelected('dashboard'));
  }

  selectable(key: SetupKey): boolean {
    if (!this.data) {
      return false;
    }

    if (key === 'dashboard') {
      return this.dashboardSelectable;
    }

    const [kind, id] = key.split(':') as ['skills' | 'mcp', HarnessId];
    const entry = this.harness(id);
    if (!entry?.installed) {
      return false;
    }

    if (kind === 'skills') {
      return entry.skills.type === 'install';
    }

    return entry.mcp.type === 'missing' && (this.data.dashboard.installed || (this.dashboardSelectable && !this.off.dashboard));
  }

  isSelected(key: SetupKey): boolean {
    return this.selectable(key) && !this.off[key];
  }

  toggle(key: SetupKey, on: boolean): void {
    this.off = { ...this.off, [key]: !on };
  }

  get selectedKeys(): SetupKey[] {
    const data = this.data;
    if (!data) {
      return [];
    }

    const keys: SetupKey[] = ['dashboard'];
    for (const entry of data.harnesses) {
      keys.push(`skills:${entry.harness}`);
    }
    for (const entry of data.harnesses) {
      keys.push(`mcp:${entry.harness}`);
    }

    return keys.filter((key) => this.isSelected(key));
  }

  /** An item that was chosen and is waiting for its turn in a run. */
  waiting(key: SetupKey): boolean {
    return this.running && this.phase(key) === 'idle' && this.picked.has(key);
  }

  /** The items of the run in progress. */
  private picked = $state.raw(new Set<SetupKey>());

  // ── Outcome ──────────────────────────────────────────────────────────────

  /** Something was set up in this visit. */
  get anyDone(): boolean {
    return Object.values(this.phases).includes('done');
  }

  /** Every harness that is installed has everything: nothing is left that this step could do or point to. */
  get allConfigured(): boolean {
    const data = this.data;
    if (!data) {
      return false;
    }

    const mcpDone = (entry: HarnessSetup) => entry.mcp.type === 'configured' || entry.mcp.type === 'disabled';

    return data.dashboard.installed && data.harnesses.every((entry) => !entry.installed || (entry.skills.type === 'installed' && mcpDone(entry)));
  }

  /** The footer reads "Continue" rather than "Skip for now". */
  get settled(): boolean {
    return this.anyDone || this.allConfigured;
  }

  /** The commands and files the current selection would run and edit. */
  get plan(): Plan {
    const data = this.data;
    const plan: Plan = { commands: [], files: [], sideEffects: [] };
    if (!data) {
      return plan;
    }

    if (this.isSelected('dashboard')) {
      plan.commands.push({ owner: 'dashboard', command: data.dashboard.command });
      plan.sideEffects = data.dashboard.sideEffects;
    }

    for (const entry of data.harnesses) {
      if (this.isSelected(`skills:${entry.harness}`) && entry.skills.type === 'install') {
        plan.commands.push(...entry.skills.actions.map((action) => ({ owner: entry.harness, command: commandOf(entry.harness, action) })));
      }
    }

    for (const entry of data.harnesses) {
      if (this.isSelected(`mcp:${entry.harness}`) && entry.mcp.type === 'missing') {
        plan.files.push({ file: entry.mcp.file, creates: !entry.mcp.exists });
      }
    }

    return plan;
  }

  // ── Running ──────────────────────────────────────────────────────────────

  /** Opens the confirmation for the current selection; nothing runs yet. */
  ask(): void {
    if (!this.running && this.selectedKeys.length > 0) {
      this.confirming = true;
    }
  }

  /** Runs the confirmed selection: the dashboard first, then everything else together. A failure stops only what depends on it. */
  async run(): Promise<void> {
    const data = this.data;
    if (!data || this.running) {
      return;
    }

    const keys = this.selectedKeys;
    if (keys.length === 0) {
      return;
    }

    this.running = true;
    this.hooks.busy(true);
    this.picked = new Set(keys);
    this.phases = { ...this.phases, ...Object.fromEntries(keys.map((key) => [key, 'idle' as ItemPhase])) };
    this.failures = {};

    try {
      const dashboardOk = this.picked.has('dashboard') ? await this.installDashboard() : true;

      // The dashboard goes first (an agent entry needs it); after it nothing depends on anything else, so every
      // harness's skills and agent entry run side by side and show their spinners together.
      await Promise.all([
        ...data.harnesses.filter((entry) => this.picked.has(`skills:${entry.harness}`)).map((entry) => this.installSkills(entry)),
        ...data.harnesses.filter((entry) => this.picked.has(`mcp:${entry.harness}`)).map((entry) => this.registerMcp(entry.harness, dashboardOk)),
      ]);
    } finally {
      this.picked = new Set();
      await this.refresh();
      await this.hooks.companionsChanged().catch(() => {});
      this.running = false;
      this.hooks.busy(false);
    }
  }

  private set(key: SetupKey, phase: ItemPhase, failure?: string): void {
    this.phases = { ...this.phases, [key]: phase };

    if (failure !== undefined) {
      this.failures = { ...this.failures, [key]: failure };
    }
  }

  private append(line: InstallLogLine): void {
    this.log = [...this.log, line].slice(-LOG_LINES);
  }

  private async installDashboard(): Promise<boolean> {
    this.set('dashboard', 'running');
    this.log = [];
    this.installResult = null;

    try {
      const result = await this.transport.invoke('setup_install_dashboard', { confirmed: true, onLog: (line) => this.append(line) });
      this.installResult = result;

      if (result.ok) {
        this.set('dashboard', 'done');

        return true;
      }

      this.set('dashboard', 'failed', result.summary ?? '');
    } catch (error) {
      this.set('dashboard', 'failed', IpcFailure.from(error).message);
    }

    return false;
  }

  private async installSkills(entry: HarnessSetup): Promise<void> {
    const key: SetupKey = `skills:${entry.harness}`;
    if (entry.skills.type !== 'install') {
      return;
    }

    this.set(key, 'running');
    let pluginChanged = false;

    try {
      for (const action of entry.skills.actions) {
        const result = await this.transport.invoke('extension_action', { harness: entry.harness, action, dryRun: false, project: null });
        const failed = failureLine(result);

        if (failed !== null) {
          this.set(key, 'failed', failed);

          return;
        }

        pluginChanged ||= action.type.startsWith('plugin');
      }

      this.set(key, 'done');
    } catch (error) {
      this.set(key, 'failed', IpcFailure.from(error).message);
    } finally {
      // Open chats of this harness only learn about a new plugin when they restart.
      if (pluginChanged) {
        await this.hooks.chatsChanged(entry.harness).catch(() => {});
      }
    }
  }

  private async registerMcp(harness: HarnessId, dashboardOk: boolean): Promise<void> {
    const key: SetupKey = `mcp:${harness}`;
    if (!dashboardOk) {
      this.set(key, 'skipped');

      return;
    }

    this.set(key, 'running');

    try {
      const state = await this.transport.invoke('setup_register_mcp', { harness });

      if (state.type === 'configured') {
        this.set(key, 'done');
      } else if (state.type === 'refused' && state.file !== '') {
        this.set(key, 'failed', refusalText(state.reason));
      } else {
        this.set(key, 'failed', t('onboarding.setup.failed.mcp'));
      }
    } catch (error) {
      this.set(key, 'failed', IpcFailure.from(error).message);
    }
  }
}

export const setup = new SetupController(ipc, {
  busy: (busy) => (onboarding.busy = busy),
  chatsChanged: reloadChatsAfterPluginChange,
  companionsChanged: () => ecosystem.load(true),
});

import { t } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { failureLine } from '../extensions/actions/action-model';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { CompanionInstaller } from './companion-installer.svelte';
import { onboarding } from './onboarding-state.svelte';
import { commandOf, companionEffects, MCP_COMPANION, refusalText, type Companion, type ItemPhase, type McpKind, type Plan, type PlannedFile, type SetupKey } from './setup-model';

export type SetupLoad = 'idle' | 'loading' | 'ready' | 'error';

interface Hooks {
  /** Told when a run starts and ends, so the flow cannot be left meanwhile. */
  busy: (busy: boolean) => void;
  /** After plugin changes, so open chats pick them up: restarts the harness's idle chats and says how many. */
  chatsChanged: (harness: HarnessId) => Promise<number>;
  /** After a run, so the Ecosystem screen shows the new state. */
  companionsChanged: () => Promise<void>;
  /** After the memory server was registered in a harness, which turns Project memory on, so Settings shows it. */
  memoryChanged: () => Promise<void>;
}

/**
 * The recommended setup, shown on two steps: "Meet the Harness family" reads the machine and describes what each member
 * would change, "Set up" runs it. Opening only reads; nothing runs until the user has seen the
 * list of commands and files and confirmed it. A run goes in a fixed order: the dashboard, then the memory server,
 * then the skills of each harness and, per harness, the dashboard's entry and then the memory server's entry in its
 * MCP file.
 */
export class SetupController {
  data = $state.raw<RecommendedSetup | null>(null);
  load = $state<SetupLoad>('idle');
  error = $state<string | null>(null);
  running = $state(false);
  /** The confirmation dialog is open. */
  confirming = $state(false);
  /** Open chats restarted so they load new plugins; the Done step says so instead of a toast. */
  chatsRestarted = $state(0);

  /** Items the user switched off; everything that can be set up starts on. */
  private off = $state<Record<string, boolean>>({});
  private phases = $state<Record<string, ItemPhase>>({});
  private failures = $state<Record<string, string>>({});

  /** The two installers' output and outcome, each shown on its own card. */
  readonly installers: Record<Companion, CompanionInstaller>;

  private token = 0;

  constructor(
    private readonly transport: Transport,
    private readonly hooks: Hooks,
  ) {
    this.installers = { dashboard: new CompanionInstaller('dashboard', transport), memory: new CompanionInstaller('memory', transport) };
  }

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
    this.chatsRestarted = 0;
    this.off = {};
    this.phases = {};
    this.failures = {};
    this.installers.dashboard.reset();
    this.installers.memory.reset();

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

  /** The skills of every harness, which the step shows as one item: the Harness Skills row. */
  private get skillKeys(): SetupKey[] {
    return (this.data?.harnesses ?? []).map((entry) => `skills:${entry.harness}` as SetupKey);
  }

  /** The skills row's phase across harnesses: working while any is, failed once one failed, done when some finished. */
  skillsShown(): ItemPhase {
    const phases = this.skillKeys.map((key) => this.shown(key));

    for (const phase of ['running', 'failed', 'done'] as const) {
      if (phases.includes(phase)) {
        return phase;
      }
    }

    return 'idle';
  }

  skillsWaiting(): boolean {
    return this.skillKeys.some((key) => this.waiting(key));
  }

  /** What went wrong in each harness the skills did not install in. */
  skillsFailures(): { harness: HarnessId; reason: string }[] {
    return (this.data?.harnesses ?? []).flatMap((entry) => {
      const reason = this.failure(`skills:${entry.harness}`);

      return reason === null ? [] : [{ harness: entry.harness, reason }];
    });
  }

  // ── Selection ────────────────────────────────────────────────────────────

  /** Whether a companion's installer can be chosen: it is missing and runs on this system. */
  companionSelectable(companion: Companion): boolean {
    const server = this.data?.[companion];

    return !!server && !server.installed && server.supportedOnThisOs;
  }

  selectable(key: SetupKey): boolean {
    if (!this.data) {
      return false;
    }

    if (key === 'dashboard' || key === 'memory') {
      return this.companionSelectable(key);
    }

    const [kind, id] = key.split(':') as ['skills' | McpKind, HarnessId];
    const entry = this.harness(id);
    if (!entry?.installed) {
      return false;
    }

    if (kind === 'skills') {
      return entry.skills.type === 'install';
    }

    // An MCP entry runs its companion, so that has to be installed already or be part of the run.
    const companion = MCP_COMPANION[kind];

    return entry[kind].type === 'missing' && (this.data[companion].installed || (this.companionSelectable(companion) && !this.off[companion]));
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

    const keys: SetupKey[] = ['dashboard', 'memory'];
    for (const entry of data.harnesses) {
      keys.push(`skills:${entry.harness}`);
    }
    for (const entry of data.harnesses) {
      keys.push(`mcp:${entry.harness}`);
    }
    for (const entry of data.harnesses) {
      keys.push(`memoryMcp:${entry.harness}`);
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

  /** A run left something undone: an item failed, or was skipped because what it needs failed. */
  get hasFailed(): boolean {
    return Object.values(this.phases).some((phase) => phase === 'failed' || phase === 'skipped');
  }

  /** The step's forward action: install what is missing and go on, go on past a failed run, or just go on. */
  get forward(): 'install' | 'anyway' | 'continue' {
    // While a run is going the button still reads as the install it is carrying out.
    if (this.hasFailed && !this.running) {
      return 'anyway';
    }

    return this.selectedKeys.length > 0 ? 'install' : 'continue';
  }

  /** The commands and files the current selection would run and edit. */
  get plan(): Plan {
    const data = this.data;
    const plan: Plan = { commands: [], files: [], effects: [] };
    if (!data) {
      return plan;
    }

    for (const companion of ['dashboard', 'memory'] as const) {
      if (this.isSelected(companion)) {
        plan.commands.push({ owner: companion, command: data[companion].command });
        plan.effects.push({ owner: companion, effects: companionEffects(companion, data[companion]) });
      }
    }

    for (const entry of data.harnesses) {
      if (this.isSelected(`skills:${entry.harness}`) && entry.skills.type === 'install') {
        plan.commands.push(...entry.skills.actions.map((action) => ({ owner: entry.harness, command: commandOf(entry.harness, action) })));
      }
    }

    // Both servers usually go into the same file, which is then listed once with both entries.
    const files = new Map<string, PlannedFile>();
    for (const kind of ['mcp', 'memoryMcp'] as const) {
      for (const entry of data.harnesses) {
        const mcp = entry[kind];
        if (!this.isSelected(`${kind}:${entry.harness}`) || mcp.type !== 'missing') {
          continue;
        }

        const planned = files.get(mcp.file);
        if (planned) {
          planned.entries += 1;
        } else {
          files.set(mcp.file, { file: mcp.file, creates: !mcp.exists, entries: 1 });
        }
      }
    }
    plan.files.push(...files.values());

    return plan;
  }

  // ── Running ──────────────────────────────────────────────────────────────

  /** Opens the confirmation for the current selection; nothing runs yet. */
  ask(): void {
    if (!this.running && this.selectedKeys.length > 0) {
      this.confirming = true;
    }
  }

  /** Runs the confirmed selection: the dashboard, then the memory server, then everything else together. A failure stops only what depends on it. */
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
      // Everything that does not depend on something else runs at once. The two installers stay one after the other:
      // on Windows both edit the user's PATH, and two writers at a time could lose one of the entries. The skills need
      // neither installer, so they start right away. A harness's MCP entry waits only for its own server's installer,
      // and its two entries go into the same file, so the memory server's waits for the dashboard's.
      const dashboard = this.picked.has('dashboard') ? this.installCompanion('dashboard') : Promise.resolve(true);
      const memory = dashboard.then(() => (this.picked.has('memory') ? this.installCompanion('memory') : true));

      await Promise.all([
        dashboard,
        memory,
        ...data.harnesses.filter((entry) => this.picked.has(`skills:${entry.harness}`)).map((entry) => this.installSkills(entry)),
        ...data.harnesses.map(async (entry) => {
          if (this.picked.has(`mcp:${entry.harness}`)) {
            await this.registerMcp('mcp', entry.harness, await dashboard);
          }
          if (this.picked.has(`memoryMcp:${entry.harness}`)) {
            await this.registerMcp('memoryMcp', entry.harness, await memory);
          }
        }),
      ]);
    } finally {
      const memoryRegistered = [...this.picked].some((key) => key.startsWith('memoryMcp:') && this.phase(key) === 'done');
      this.picked = new Set();
      // The rows already show the outcome, so the flow is free at once. Re-reading the machine and letting Settings
      // and the Ecosystem screen catch up all happen together in the background.
      this.running = false;
      this.hooks.busy(false);
      void Promise.all([
        this.refresh(),
        memoryRegistered ? this.hooks.memoryChanged().catch(() => {}) : null,
        this.hooks.companionsChanged().catch(() => {}),
      ]);
    }
  }

  private set(key: SetupKey, phase: ItemPhase, failure?: string): void {
    this.phases = { ...this.phases, [key]: phase };

    if (failure !== undefined) {
      this.failures = { ...this.failures, [key]: failure };
    }
  }

  private async installCompanion(companion: Companion): Promise<boolean> {
    this.set(companion, 'running');

    const outcome = await this.installers[companion].run();

    if (outcome.ok) {
      this.set(companion, 'done');
    } else {
      this.set(companion, 'failed', outcome.failure);
    }

    return outcome.ok;
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
        // In the background: restarting chats must not hold up the run; the Done step shows the count when it is in.
        void this.hooks.chatsChanged(entry.harness).then(
          (count) => (this.chatsRestarted += count),
          () => {},
        );
      }
    }
  }

  /** Adds one MCP entry; one whose companion did not install is skipped, like the dashboard's entry always was. */
  private async registerMcp(kind: McpKind, harness: HarnessId, companionOk: boolean): Promise<void> {
    const key: SetupKey = `${kind}:${harness}`;
    if (!companionOk) {
      this.set(key, 'skipped');

      return;
    }

    this.set(key, 'running');

    try {
      const state = await (kind === 'mcp' ? this.transport.invoke('setup_register_mcp', { harness }) : this.transport.invoke('setup_register_memory', { harness }));

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
  chatsChanged: (harness) => agentSessions.reloadIdle(harness),
  companionsChanged: () => ecosystem.load(true),
  memoryChanged: () => prefs.syncServerManaged(),
});

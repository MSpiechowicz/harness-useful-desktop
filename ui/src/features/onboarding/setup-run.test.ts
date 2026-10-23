import { describe, expect, it } from 'vitest';

import type { DashboardInstallResult } from '$lib/ipc/bindings/DashboardInstallResult';
import type { DashboardSetup } from '$lib/ipc/bindings/DashboardSetup';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { McpState } from '$lib/ipc/bindings/McpState';
import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';
import type { CommandName } from '$lib/ipc/commands';
import type { Transport } from '$lib/ipc/transport';

import { SetupController } from './setup-state.svelte';

const missing: McpState = { type: 'missing', file: '~/mcp.json', exists: true };
const server: DashboardSetup = { installed: true, version: '1.0.0', supportedOnThisOs: true, command: 'install', sideEffects: [] };

const entry = (id: HarnessId, skills: HarnessSetup['skills']): HarnessSetup => ({ harness: id, installed: true, skills, mcp: missing, memoryMcp: missing, desktopMcp: missing });

const setupData: RecommendedSetup = {
  dashboard: server,
  memory: server,
  harnesses: [
    entry('omp', { type: 'installed', version: null }),
    entry('opencode', { type: 'script', command: 'install.sh', sideEffects: [], outdatedFrom: null }),
    entry('codex', { type: 'script', command: 'install.sh', sideEffects: [], outdatedFrom: null }),
  ],
};

const installResult = (ok: boolean): DashboardInstallResult => ({ ok, summary: ok ? null : 'could not download', exitCode: ok ? 0 : 1, timedOut: false, truncated: false, version: null, command: 'install.sh' });

/** A transport whose OpenCode and Codex skills installers end only when the test says so. */
function fixture(options: { holdCodex?: boolean } = {}) {
  const events: string[] = [];
  let finishSkills: (ok: boolean) => void = () => {};
  const skills = new Promise<boolean>((resolve) => (finishSkills = resolve));
  let finishCodex: (ok: boolean) => void = () => {};
  // Codex's installer ends at once unless the test holds it.
  const codex = new Promise<boolean>((resolve) => (finishCodex = resolve));
  if (!options.holdCodex) {
    finishCodex(true);
  }

  const transport: Transport = {
    invoke: (async (name: CommandName, args: unknown) => {
      switch (name) {
        case 'setup_status':
          return setupData;
        case 'setup_install_opencode_skills':
          events.push('skills:start');
          return installResult(await skills);
        case 'setup_install_codex_skills':
          events.push('codex-skills:start');
          return installResult(await codex);
        case 'setup_register_mcp':
        case 'setup_register_memory':
        case 'setup_register_desktop_mcp':
          events.push(`${name}:${(args as { harness: HarnessId }).harness}`);
          return { type: 'configured', file: '~/mcp.json', name: 'x' } satisfies McpState;
        default:
          throw new Error(`unexpected ${name}`);
      }
    }) as Transport['invoke'],
  };

  const controller = new SetupController(transport, { busy: () => {}, chatsChanged: async () => 0, companionsChanged: async () => {}, memoryChanged: async () => {} });

  return { controller, events, finishSkills: (ok: boolean) => finishSkills(ok), finishCodex: (ok: boolean) => finishCodex(ok) };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A harness's MCP entries in the order they were registered. */
const entriesOf = (events: string[], harness: HarnessId) => events.filter((event) => event.endsWith(`:${harness}`));

describe('A setup run with OpenCode’s skills script', () => {
  it('registers OpenCode’s MCP entries only once the skills script has ended, while other harnesses do not wait', async () => {
    const { controller, events, finishSkills } = fixture();
    await controller.open();

    const run = controller.run();
    await flush();

    expect(events).toContain('skills:start');
    expect(events).toContain('setup_register_mcp:omp');
    expect(events).toContain('setup_register_desktop_mcp:omp');
    expect(events.filter((event) => event.endsWith(':opencode'))).toEqual([]);

    finishSkills(true);
    await run;

    expect(events.filter((event) => event.endsWith(':opencode'))).toEqual(['setup_register_mcp:opencode', 'setup_register_memory:opencode', 'setup_register_desktop_mcp:opencode']);
    expect(events.indexOf('setup_register_mcp:opencode')).toBeGreaterThan(events.indexOf('skills:start'));
  });

  it('still registers OpenCode’s MCP entries when the skills script fails', async () => {
    const { controller, events, finishSkills } = fixture();
    await controller.open();

    const run = controller.run();
    await flush();
    expect(events.filter((event) => event.endsWith(':opencode'))).toEqual([]);

    finishSkills(false);
    await run;

    expect(controller.phase('skills:opencode')).toBe('failed');
    expect(events.filter((event) => event.endsWith(':opencode'))).toEqual(['setup_register_mcp:opencode', 'setup_register_memory:opencode', 'setup_register_desktop_mcp:opencode']);
    expect(controller.phase('mcp:opencode')).toBe('done');
  });

  it('runs Codex’s skills through its own installer and registers its MCP entries once that has ended', async () => {
    const { controller, events, finishSkills, finishCodex } = fixture({ holdCodex: true });
    await controller.open();

    const run = controller.run();
    await flush();

    expect(events).toContain('codex-skills:start');
    expect(entriesOf(events, 'codex')).toEqual([]);

    finishSkills(true);
    await flush();
    expect(entriesOf(events, 'codex')).toEqual([]);

    finishCodex(true);
    await run;

    expect(controller.phase('skills:codex')).toBe('done');
    expect(entriesOf(events, 'codex')).toEqual(['setup_register_mcp:codex', 'setup_register_memory:codex', 'setup_register_desktop_mcp:codex']);
    expect(events.indexOf('setup_register_mcp:codex')).toBeGreaterThan(events.indexOf('codex-skills:start'));
  });

  it('still registers Codex’s MCP entries when its skills script fails, and says which one failed', async () => {
    const { controller, events, finishSkills, finishCodex } = fixture({ holdCodex: true });
    await controller.open();

    const run = controller.run();
    await flush();
    finishSkills(true);
    finishCodex(false);
    await run;

    expect(controller.phase('skills:codex')).toBe('failed');
    expect(controller.phase('skills:opencode')).toBe('done');
    expect(controller.skillsFailures()).toEqual([{ harness: 'codex', reason: 'could not download' }]);
    expect(entriesOf(events, 'codex')).toEqual(['setup_register_mcp:codex', 'setup_register_memory:codex', 'setup_register_desktop_mcp:codex']);
  });
});

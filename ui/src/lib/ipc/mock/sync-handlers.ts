import type { SyncExportPreview } from '../bindings/SyncExportPreview';
import type { SyncImportPlan } from '../bindings/SyncImportPlan';
import type { SyncImportSection } from '../bindings/SyncImportSection';
import type { SyncItemOutcome } from '../bindings/SyncItemOutcome';
import type { SyncOutcome } from '../bindings/SyncOutcome';
import { IpcFailure } from '../transport';
import { EXCLUDED_ALL, EXCLUDED_TYPICAL, EXPORT_SECTIONS, HOSTILE_PLAN_SECTIONS, HOSTILE_UNSUPPORTED, TYPICAL_PLAN_SECTIONS } from './sync-fixtures';
import type { Handlers } from './types';

type SyncCommands = 'sync_export_preview' | 'sync_export' | 'sync_export_save' | 'sync_import_preview' | 'sync_import_apply';

/**
 * What the mock share setup starts from. The browser preview picks one with `?sync=<name>`: `typical` (the default) is
 * OMP and PI (OpenCode has no share support) with marketplaces, plugins, packages and MCP servers in every kind of state, `empty` a machine with nothing
 * to share, `hostile` a bundle of refused items and an uninstalled harness, `newer` a bundle from a newer app,
 * `partial` an import where a marketplace fails and its plugins are skipped, and `cancelled` closes both pickers.
 */
export const SYNC_SCENARIOS = ['typical', 'empty', 'hostile', 'newer', 'partial', 'cancelled'] as const;
export type SyncScenario = (typeof SYNC_SCENARIOS)[number];

/** Pause between two items of an import; tests do not wait. */
const STEP_MS = import.meta.env.MODE === 'test' ? 0 : 700;

const FAILED_MARKETPLACE = 'omp:marketplace:harness-useful-skills';
const FAILED_ARGV = ['omp', 'plugin', 'marketplace', 'add', 'MSpiechowicz/harness-useful-skills'];

/** Reads the scenario from the page URL (browser preview only); unknown names fall back to the default. */
export function syncScenarioFromLocation(search: string = globalThis.location?.search ?? ''): SyncScenario {
  const name = new URLSearchParams(search).get('sync');

  return SYNC_SCENARIOS.find((scenario) => scenario === name) ?? 'typical';
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** base64url without padding, like the real code (the mock does not compress). */
function encode(text: string): string {
  return btoa(String.fromCharCode(...new TextEncoder().encode(text)))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '');
}

function sectionsFor(scenario: SyncScenario): SyncImportSection[] {
  return scenario === 'hostile' ? HOSTILE_PLAN_SECTIONS : TYPICAL_PLAN_SECTIONS;
}

function invalid(message: string): IpcFailure {
  return IpcFailure.from({ code: 'invalidInput', message });
}

/** In-memory share setup: an export of fixed items and an import whose plan is kept under an id until it is applied. */
export function syncHandlers(scenario: () => SyncScenario = syncScenarioFromLocation): Pick<Handlers, SyncCommands> {
  let plan: SyncImportPlan | null = null;
  let counter = 0;

  function preview(): SyncExportPreview {
    const current = scenario();

    if (current === 'empty') {
      return { sections: [], excluded: [] };
    }

    return { sections: EXPORT_SECTIONS, excluded: current === 'hostile' ? EXCLUDED_ALL : EXCLUDED_TYPICAL };
  }

  function checkedSelection(selected: string[]): void {
    const known = new Set(preview().sections.flatMap((section) => section.items.map((item) => item.id)));

    if (selected.length === 0 || selected.some((id) => !known.has(id))) {
      throw invalid('Choose at least one item of this machine to share.');
    }
  }

  /** What an apply does to one item: a failed marketplace fails, its plugins are skipped, the rest succeeds. */
  function outcomeOf(section: SyncImportSection, id: string, failed: Set<string>): { outcome: SyncOutcome; summary: string | null; argv: string[] } {
    const item = section.items.find((candidate) => candidate.id === id)!;
    const argv = item.steps.flatMap((step) => (step.type === 'cli' ? step.command : []));

    if (scenario() === 'partial' && id === FAILED_MARKETPLACE) {
      failed.add(id);

      return { outcome: { type: 'failed' }, summary: 'Could not clone the repository: exit code 128.', argv: FAILED_ARGV };
    }

    if (item.dependsOn && failed.has(item.dependsOn)) {
      return { outcome: { type: 'skipped', reason: { type: 'dependencyFailed' } }, summary: null, argv: [] };
    }

    return { outcome: { type: 'done' }, summary: null, argv };
  }

  return {
    sync_export_preview: async () => preview(),

    sync_export: async ({ selected }) => {
      checkedSelection(selected);

      const code = `hud1:${encode(JSON.stringify({ format: 'harness-useful-desktop/setup', version: 1, items: selected }))}`;

      return { code, itemCount: selected.length, bytes: code.length };
    },

    sync_export_save: async ({ selected }) => {
      checkedSelection(selected);

      return scenario() === 'cancelled' ? null : '/home/demo/Documents/harness-setup-2026-10-15.json';
    },

    sync_import_preview: async ({ input }) => {
      const current = scenario();

      if (input.type === 'file' && current === 'cancelled') {
        return null;
      }

      if (input.type === 'text') {
        const text = input.text.trim();

        if (!text.startsWith('hud1:') && !text.startsWith('{')) {
          throw invalid('This is not a Harness Desktop setup. A setup code starts with “hud1:”, a setup file with “{”.');
        }
      }

      if (current === 'newer') {
        throw IpcFailure.from({ code: 'unsupported', message: 'This setup was made by a newer version of Harness Desktop. Update the app to import it.' });
      }

      const sections = sectionsFor(current);
      const hostile = current === 'hostile';

      counter += 1;
      plan = {
        planId: `plan-${counter}`,
        app: '0.15.0',
        createdAt: '2026-10-15',
        sections,
        unsupported: hostile ? HOSTILE_UNSUPPORTED : [],
        invalidCount: hostile ? 5 : 0,
      };

      return plan;
    },

    sync_import_apply: async ({ planId, selected, confirmed, onProgress }) => {
      if (!confirmed) {
        throw IpcFailure.from({ code: 'forbidden', message: 'An import runs only after the user confirmed it.' });
      }

      if (!plan || plan.planId !== planId) {
        throw invalid('This import is no longer valid. Review the setup again.');
      }

      const current = plan;
      const allowed = new Set(current.sections.flatMap((section) => section.items.filter((item) => item.selectable).map((item) => item.id)));

      if (selected.length === 0 || selected.some((id) => !allowed.has(id))) {
        throw invalid('Only items that can be imported can be selected.');
      }

      // The plan is consumed: applying it twice is refused.
      plan = null;

      const failed = new Set<string>();
      const items: SyncItemOutcome[] = [];
      const ordered = current.sections.flatMap((section) => section.items.filter((item) => selected.includes(item.id)).map((item) => ({ section, item })));

      for (const [index, { section, item }] of ordered.entries()) {
        onProgress({ type: 'started', id: item.id, index, total: ordered.length });
        await delay(STEP_MS);

        const { outcome, summary, argv } = outcomeOf(section, item.id, failed);
        const finished: SyncItemOutcome = { id: item.id, harness: section.harness, kind: item.kind, label: item.label, outcome, summary, argv };

        items.push(finished);
        onProgress({ type: 'finished', outcome: finished });
      }

      return { ok: items.every((item) => item.outcome.type === 'done' || item.outcome.type === 'alreadyPresent'), items };
    },
  };
}

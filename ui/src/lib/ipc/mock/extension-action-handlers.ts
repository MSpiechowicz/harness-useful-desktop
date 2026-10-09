import type { CliResult } from '../bindings/CliResult';
import type { ExtensionAction } from '../bindings/ExtensionAction';
import type { ExtensionsView } from '../bindings/ExtensionsView';
import type { HarnessId } from '../bindings/HarnessId';
import { IpcFailure } from '../transport';
import type { MockState } from './state';
import type { Handlers } from './types';

type ActionCommands = 'extension_action' | 'extensions_view';

/** A real run takes a moment; tests do not wait for it. */
const RUN_MS = import.meta.env.MODE === 'test' ? 0 : 700;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

interface Applied {
  harness: HarnessId;
  action: ExtensionAction;
  project: string | null;
}

/** The harness CLI's arguments for an action, as the backend maps them (program name first). */
function argvOf(harness: HarnessId, action: ExtensionAction, project: string | null): string[] {
  const local = project ? ['--local'] : [];

  switch (action.type) {
    case 'pluginInstall':
      return ['omp', 'plugin', 'install', action.target, '--scope', action.scope];
    case 'pluginUninstall':
      return ['omp', 'plugin', 'uninstall', action.name];
    case 'pluginEnable':
      return ['omp', 'plugin', 'enable', action.name];
    case 'pluginDisable':
      return ['omp', 'plugin', 'disable', action.name];
    case 'pluginUpgrade':
      return ['omp', 'plugin', 'upgrade', ...(action.name ? [action.name] : [])];
    case 'marketplaceAdd':
      return ['omp', 'plugin', 'marketplace', 'add', action.source];
    case 'marketplaceRemove':
      return ['omp', 'plugin', 'marketplace', 'remove', action.name];
    case 'marketplaceUpdate':
      return ['omp', 'plugin', 'marketplace', 'update', ...(action.name ? [action.name] : [])];
    case 'packageInstall':
      return ['pi', 'install', action.source, ...local];
    case 'packageRemove':
      return ['pi', 'remove', action.source, ...local];
    case 'packageUpdate':
      return harness === 'pi' && action.source ? ['pi', 'update', action.source] : ['pi', 'update', '--extensions'];
  }
}

/** OMP previews installs and uninstalls only; PI previews nothing. */
function canPreview(harness: HarnessId, action: ExtensionAction): boolean {
  return harness === 'omp' && (action.type === 'pluginInstall' || action.type === 'pluginUninstall');
}

function targetOf(action: ExtensionAction): string {
  switch (action.type) {
    case 'pluginInstall':
      return action.target;
    case 'pluginUninstall':
    case 'pluginEnable':
    case 'pluginDisable':
    case 'marketplaceRemove':
      return action.name;
    case 'marketplaceAdd':
    case 'packageInstall':
    case 'packageRemove':
      return action.source;
    case 'pluginUpgrade':
    case 'marketplaceUpdate':
      return action.name ?? 'everything';
    case 'packageUpdate':
      return action.source ?? 'every package';
  }
}

const PAST: Record<ExtensionAction['type'], string> = {
  pluginInstall: 'Installed',
  pluginUninstall: 'Uninstalled',
  pluginEnable: 'Enabled',
  pluginDisable: 'Disabled',
  pluginUpgrade: 'Upgraded',
  marketplaceAdd: 'Added marketplace',
  marketplaceRemove: 'Removed marketplace',
  marketplaceUpdate: 'Updated marketplace',
  packageInstall: 'Installed',
  packageRemove: 'Removed',
  packageUpdate: 'Updated',
};

/** The sample "broken" targets: anything named `fail…`, and the disable of the hostile fixture plugin. */
function failsFor(action: ExtensionAction): boolean {
  const target = targetOf(action);

  return target.includes('fail') || (action.type === 'pluginDisable' && target.startsWith('escaping-plugin'));
}

/** `command` is what applies the action; `argv` is what ran, which differs only for a dry run with a preview flag. */
function result(command: string[], fields: Partial<CliResult>): CliResult {
  const base: CliResult = { ok: true, summary: null, exitCode: 0, stdout: '', stderr: '', truncated: false, timedOut: false, argv: command, command };

  return { ...base, ...fields };
}

function dryRun(harness: HarnessId, action: ExtensionAction, argv: string[]): CliResult {
  if (!canPreview(harness, action)) {
    return result(argv, { summary: `${harness === 'omp' ? 'OMP' : 'PI'} has no preview for this action; nothing was run.`, exitCode: null });
  }

  const verb = action.type === 'pluginInstall' ? 'install' : 'uninstall';
  const line = `[dry-run] Would ${verb} ${targetOf(action)}`;

  return result(argv, { argv: [...argv, '--dry-run'], summary: line, stdout: `${line}\n` });
}

function run(action: ExtensionAction, argv: string[]): CliResult {
  if (failsFor(action)) {
    const verb = argv.slice(1).filter((part) => !part.startsWith('-'))[1] ?? 'run';
    const line = `✘ Failed to ${verb} ${targetOf(action)}: Error: ${targetOf(action)} not found in runtime config`;

    // OMP prints the failure and still exits with 0.
    return result(argv, { ok: false, summary: line, stderr: `${line}\n` });
  }

  const line = `✔ ${PAST[action.type]} ${targetOf(action)}`;

  return result(argv, { summary: line, stdout: `${line}\n` });
}

function basename(source: string): string {
  return source.replace(/\.git$/, '').split(/[/:]/).filter(Boolean).at(-1) ?? source;
}

/** Applies one successful action to a copy of the view, so the lists change like the real CLI's. */
export function applyAction(view: ExtensionsView, { action, project }: Pick<Applied, 'action' | 'project'>): ExtensionsView {
  const next = structuredClone(view);
  const matches = (name: string) => (plugin: { id: string; name: string }) => plugin.id === name || plugin.name === name;

  switch (action.type) {
    case 'pluginEnable':
    case 'pluginDisable':
      next.plugins.filter(matches(action.name)).forEach((plugin) => (plugin.enabled = action.type === 'pluginEnable'));
      break;

    case 'pluginUninstall': {
      const gone = next.plugins.filter(matches(action.name));
      next.plugins = next.plugins.filter((plugin) => !gone.includes(plugin));
      next.marketplaces.forEach((marketplace) =>
        marketplace.entries.filter((entry) => gone.some((plugin) => plugin.name === entry.name)).forEach((entry) => (entry.installed = false)),
      );
      break;
    }

    case 'pluginInstall': {
      const [name, marketplace = null] = action.target.split('@');
      const entry = next.marketplaces.find((candidate) => candidate.name === marketplace)?.entries.find((candidate) => candidate.name === name);
      if (entry) {
        entry.installed = true;
      }

      if (!next.plugins.some(matches(action.target))) {
        next.plugins.push({
          id: action.target,
          name,
          version: entry?.version ?? '1.0.0',
          description: entry?.description ?? null,
          enabled: true,
          scope: action.scope,
          marketplace,
          features: [],
          path: null,
          latestVersion: entry?.version ?? null,
          updateAvailable: false,
        });
      }
      break;
    }

    case 'marketplaceAdd':
      next.marketplaces.push({ name: basename(action.source), sourceType: 'github', source: action.source, addedAt: new Date().toISOString(), updatedAt: null, entries: [], error: null });
      break;

    case 'marketplaceRemove':
      next.marketplaces = next.marketplaces.filter((marketplace) => marketplace.name !== action.name);
      break;

    case 'marketplaceUpdate':
      next.marketplaces.filter((marketplace) => !action.name || marketplace.name === action.name).forEach((marketplace) => (marketplace.updatedAt = new Date().toISOString()));
      break;

    case 'packageInstall':
      next.packages.push({ source: { kind: 'text', value: action.source }, kind: action.source.startsWith('npm:') ? 'npm' : 'git', scope: project ? 'project' : 'user', resources: [] });
      break;

    case 'packageRemove':
      next.packages = next.packages.filter((pkg) => !(pkg.source.kind === 'text' && pkg.source.value === action.source));
      break;

    case 'pluginUpgrade':
    case 'packageUpdate':
      break;
  }

  return next;
}

/**
 * Extension actions that really change the mock's extension lists, so the list the UI re-reads
 * after an action shows the outcome. Wraps the plain `extensions_view` handler.
 */
export function extensionActionHandlers(state: MockState, plainView: Handlers['extensions_view']): Pick<Handlers, ActionCommands> {
  const applied: Applied[] = [];

  return {
    extension_action: async ({ harness, action, dryRun: preview, project }) => {
      const wrongHarness = harness === 'omp' ? action.type.startsWith('package') : !action.type.startsWith('package');
      if (wrongHarness) {
        throw IpcFailure.from({ code: 'unsupported', message: `${harness === 'omp' ? 'OMP installs packages as plugins' : 'PI has no plugins or marketplaces'}; use another action.` });
      }

      state.requireKnown(project);

      const argv = argvOf(harness, action, project);
      if (preview) {
        return dryRun(harness, action, argv);
      }

      await delay(RUN_MS);

      const outcome = run(action, argv);
      if (outcome.ok) {
        applied.push({ harness, action, project });
      }

      return outcome;
    },

    extensions_view: async (args) => {
      const view = await plainView(args);

      return applied.filter((entry) => entry.harness === args.harness).reduce((current, entry) => applyAction(current, entry), view);
    },
  };
}

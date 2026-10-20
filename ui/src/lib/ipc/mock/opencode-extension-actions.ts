import type { CliResult } from '../bindings/CliResult';
import type { ExtensionAction } from '../bindings/ExtensionAction';
import type { ExtensionsView } from '../bindings/ExtensionsView';
import { IpcFailure } from '../transport';

/** The global config file OpenCode plugins are listed in, as the backend shows it. */
const CONFIG_FILE = '~/.config/opencode/opencode.json';

/** The package name and the wanted version or tag of an npm spec: `@scope/name@1.2.3` is `@scope/name` and `1.2.3`. */
export function npmSpec(target: string): { name: string; wanted: string | null } {
  const at = target.startsWith('@') ? target.indexOf('@', 1) : target.indexOf('@');

  return at === -1 ? { name: target, wanted: null } : { name: target.slice(0, at), wanted: target.slice(at + 1) };
}

function shown(path: string): string {
  return path.replace(/^\/home\/demo(?=\/)/, '~');
}

function cliResult(fields: Partial<CliResult>): CliResult {
  return { ok: true, summary: null, exitCode: 0, stdout: '', stderr: '', truncated: false, timedOut: false, argv: [], command: [], ...fields };
}

function install(target: string, dryRun: boolean): CliResult {
  const command = ['opencode', 'plugin', target, '-g'];

  // OpenCode has no preview flag: a dry run starts nothing.
  if (dryRun) {
    return cliResult({ summary: 'OpenCode has no preview for this action. Nothing was run.', exitCode: null, argv: command, command });
  }

  const line = `Installed ${npmSpec(target).name}`;

  return cliResult({ summary: line, stdout: `${line}\n`, argv: command, command });
}

/**
 * Removing a plugin edits the global config file, never a CLI. A plugin that a file of its own or the project
 * defines is left to the user, as the backend does.
 */
function uninstall(name: string, dryRun: boolean, view: ExtensionsView): CliResult {
  const listed = view.plugins.find((plugin) => plugin.id === name);

  if (listed?.path) {
    const file = shown(listed.path);

    return cliResult({
      ok: false,
      summary: `${listed.name} is defined in ${file}. Remove that file yourself.`,
      exitCode: null,
      configEdit: { file, manualEdit: true },
    });
  }

  const summary = dryRun
    ? `Would remove ${name} from the plugin list in ${CONFIG_FILE}. Nothing was changed.`
    : `Removed ${name} from the plugin list in ${CONFIG_FILE}.`;

  return cliResult({ summary, exitCode: null, configEdit: { file: CONFIG_FILE, manualEdit: false } });
}

/** What OpenCode does with an extension action: install an npm plugin for the user, or remove one. Nothing else. */
export function opencodeAction(action: ExtensionAction, dryRun: boolean, view: ExtensionsView): CliResult {
  if (action.type === 'pluginInstall' && action.scope === 'user') {
    return install(action.target, dryRun);
  }

  if (action.type === 'pluginUninstall') {
    return uninstall(action.name, dryRun, view);
  }

  throw IpcFailure.from({ code: 'unsupported', message: 'OpenCode can only install plugins from npm and remove them.' });
}

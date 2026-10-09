import type { IconComponent } from '$lib/components/icon';
import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { InstallScope } from '$lib/ipc/bindings/InstallScope';

/** One button of an item's action bar. */
export interface BarAction {
  label: string;
  icon: IconComponent;
  /** Destructive actions use the quiet danger tone; the dialog repeats it on the confirm button. */
  danger?: boolean;
  request: ActionRequest;
}

/** What the install dialog asks for. */
export type ComposeKind = 'plugin' | 'marketplace' | 'package';

/** Everything the confirmation dialog needs to show before anything runs. */
export interface ActionRequest {
  harness: HarnessId;
  action: ExtensionAction;
  scope: InstallScope;
  /** The folder the command runs in for project scope; `null` for user scope. */
  project: string | null;
  /** The dialog title: "Uninstall demo-plugin". */
  title: string;
  /** The confirm button's verb. */
  verb: string;
  /** Removing or uninstalling: confirmed in the danger tone. */
  destructive: boolean;
}

interface Copy {
  verb: string;
  destructive?: boolean;
  subject: (action: ExtensionAction) => string;
}

const name = (action: ExtensionAction): string => ('name' in action && action.name ? action.name : 'everything');
const source = (action: ExtensionAction): string => ('source' in action && action.source ? action.source : 'every package');

const COPY: Record<ExtensionAction['type'], Copy> = {
  pluginInstall: { verb: 'Install', subject: (action) => ('target' in action ? action.target : '') },
  pluginUninstall: { verb: 'Uninstall', destructive: true, subject: name },
  pluginEnable: { verb: 'Enable', subject: name },
  pluginDisable: { verb: 'Disable', subject: name },
  pluginUpgrade: { verb: 'Upgrade', subject: (action) => (name(action) === 'everything' ? 'all plugins' : name(action)) },
  marketplaceAdd: { verb: 'Add marketplace', subject: source },
  marketplaceRemove: { verb: 'Remove marketplace', destructive: true, subject: name },
  marketplaceUpdate: { verb: 'Update', subject: (action) => (name(action) === 'everything' ? 'all marketplaces' : name(action)) },
  packageInstall: { verb: 'Install', subject: source },
  packageRemove: { verb: 'Remove', destructive: true, subject: source },
  packageUpdate: { verb: 'Update', subject: (action) => (source(action) === 'every package' ? 'all packages' : source(action)) },
};

/** Builds the request for one mutation; user scope never carries a project folder. */
export function actionRequest(harness: HarnessId, action: ExtensionAction, scope: InstallScope, projectPath: string | null): ActionRequest {
  const copy = COPY[action.type];
  const project = scope === 'project' ? projectPath : null;

  return {
    harness,
    action,
    scope,
    project,
    verb: copy.verb,
    destructive: copy.destructive ?? false,
    title: `${copy.verb} ${copy.subject(action)}`,
  };
}

/** The line that says what the command acts on, for the dialog. */
export function scopeSentence(request: ActionRequest, projectName: string | null): string {
  if (request.scope === 'project') {
    return `Project scope: runs inside ${projectName ?? 'the open project'} and changes only that project.`;
  }

  return 'User scope: applies to every project of this account.';
}

/** The line of a result that reports a failure, or null. OMP reports failures with exit code 0. */
export function failureLine(result: CliResult): string | null {
  if (!result.ok) {
    return result.summary ?? 'The command failed.';
  }

  const lines = [...result.stderr.split('\n'), ...result.stdout.split('\n')].map((line) => line.trim());

  return lines.find((line) => line.startsWith('✘') || line.includes('Failed to')) ?? null;
}

/** True when the CLI could not preview the action and nothing ran. */
export function hasNoPreview(result: CliResult): boolean {
  return result.exitCode === null && !result.timedOut;
}

export type Outcome = 'confirmed' | 'missing' | 'unchecked';

function pluginMatches(view: ExtensionsView, wanted: string) {
  const bare = wanted.split('@')[0];

  return view.plugins.filter((plugin) => plugin.id === wanted || plugin.name === wanted || plugin.name === bare);
}

/** Whether the re-read list shows the change the action was meant to make. */
export function outcomeIn(view: ExtensionsView, action: ExtensionAction): Outcome {
  const verdict = (shown: boolean): Outcome => (shown ? 'confirmed' : 'missing');

  switch (action.type) {
    case 'pluginEnable':
      return verdict(pluginMatches(view, action.name).some((plugin) => plugin.enabled));
    case 'pluginDisable':
      return verdict(pluginMatches(view, action.name).some((plugin) => !plugin.enabled));
    case 'pluginUninstall':
      return verdict(pluginMatches(view, action.name).length === 0);
    case 'pluginInstall':
      return verdict(pluginMatches(view, action.target).length > 0);
    case 'marketplaceAdd':
      return verdict(view.marketplaces.some((marketplace) => marketplace.source === action.source || action.source.includes(marketplace.name)));
    case 'marketplaceRemove':
      return verdict(!view.marketplaces.some((marketplace) => marketplace.name === action.name));
    case 'packageInstall':
      return verdict(view.packages.some((pkg) => pkg.source.kind === 'text' && pkg.source.value === action.source));
    case 'packageRemove':
      return verdict(!view.packages.some((pkg) => pkg.source.kind === 'text' && pkg.source.value === action.source));
    case 'pluginUpgrade':
    case 'marketplaceUpdate':
    case 'packageUpdate':
      return 'unchecked';
  }
}

export const OUTCOME_TEXT: Record<Outcome, string> = {
  confirmed: 'Confirmed: the refreshed list shows the change.',
  missing: 'The refreshed list does not show the change yet. Some changes apply after the harness restarts.',
  unchecked: 'The list was refreshed.',
};

/** Mirrors the backend's argument rules, so a bad value is caught before a round trip. */
export function targetProblem(value: string): string | null {
  const text = value.trim();

  if (text === '') {
    return 'Enter a value.';
  }
  if (text.length > 512) {
    return 'That is too long (at most 512 characters).';
  }
  if (text.startsWith('-')) {
    return 'It must not start with a dash.';
  }
  if (!/^[\w@./:+=,-]+$/.test(text)) {
    return 'Use only letters, digits and @ . _ / : + = , -';
  }
  if (/:\/\/[^/]*@/.test(text)) {
    return 'Do not put credentials in the address.';
  }

  return null;
}

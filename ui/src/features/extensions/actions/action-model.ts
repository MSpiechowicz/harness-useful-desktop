import { t } from '$lib/i18n/i18n.svelte';
import type { IconComponent } from '$lib/components/icon';
import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { InstallScope } from '$lib/ipc/bindings/InstallScope';

/** One button of an item's action bar. */
export interface BarAction {
  /** The button text. */
  label: string;
  /** The row's heading and its one-line explanation, shown beside the button. */
  title: string;
  description: string;
  icon: IconComponent;
  /** Destructive actions use the quiet danger tone; the dialog repeats it on the confirm button. */
  danger?: boolean;
  /** `ready`: there is something to do, so the button is the blue primary one. `idle`: nothing to do, so it stays but is greyed out. */
  state?: 'ready' | 'idle';
  request: ActionRequest;
}

/**
 * What the user can ask for through a confirmation. A marketplace catalog is never refreshed by hand: the updater
 * re-reads every catalog by itself (see `extension-updater.ts`), so that action has no dialog.
 */
export type UserAction = Exclude<ExtensionAction, { type: 'marketplaceUpdate' }>;

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

/** The confirm button's verb and the dialog title of one kind of action, both in the language on screen. */
interface Copy {
  verb: () => string;
  destructive?: boolean;
  title: (action: UserAction) => string;
}

const verb = (key: 'install' | 'uninstall' | 'enable' | 'disable' | 'update' | 'remove' | 'addMarketplace' | 'removeMarketplace') => () =>
  t(`extensions.verb.${key}`);

const name = (action: UserAction): string | null => ('name' in action && action.name ? action.name : null);
const source = (action: UserAction): string | null => ('source' in action && action.source ? action.source : null);

/** A title of the form "Uninstall {subject}", where the subject is the action's name, or "everything" without one. */
const named = (key: 'uninstall' | 'enable' | 'disable') => (action: UserAction) =>
  t(`extensions.title.${key}`, { subject: name(action) ?? t('extensions.subject.everything') });

const sourced = (key: 'installPackage' | 'removePackage') => (action: UserAction) =>
  t(`extensions.title.${key}`, { subject: source(action) ?? t('extensions.subject.everyPackage') });

const COPY: Record<UserAction['type'], Copy> = {
  pluginInstall: { verb: verb('install'), title: (action) => t('extensions.title.install', { subject: 'target' in action ? action.target : '' }) },
  pluginUninstall: { verb: verb('uninstall'), destructive: true, title: named('uninstall') },
  pluginEnable: { verb: verb('enable'), title: named('enable') },
  pluginDisable: { verb: verb('disable'), title: named('disable') },
  pluginUpgrade: {
    verb: verb('update'),
    title: (action) => {
      const subject = name(action);

      return subject ? t('extensions.title.upgrade', { subject }) : t('extensions.title.upgradeAll');
    },
  },
  marketplaceAdd: {
    verb: verb('addMarketplace'),
    title: (action) => t('extensions.title.addMarketplace', { subject: source(action) ?? t('extensions.subject.everyPackage') }),
  },
  marketplaceRemove: {
    verb: verb('removeMarketplace'),
    destructive: true,
    title: (action) => t('extensions.title.removeMarketplace', { subject: name(action) ?? t('extensions.subject.everything') }),
  },
  packageInstall: { verb: verb('install'), title: sourced('installPackage') },
  packageRemove: { verb: verb('uninstall'), destructive: true, title: sourced('removePackage') },
  packageUpdate: {
    verb: verb('update'),
    title: (action) => {
      const subject = source(action);

      return subject ? t('extensions.title.updatePackage', { subject }) : t('extensions.title.updateAllPackages');
    },
  },
};

/**
 * Builds the request for one mutation; user scope never carries a project folder. The dialog's title and verb are
 * read when shown, so an open dialog follows a language switch.
 */
export function actionRequest(harness: HarnessId, action: UserAction, scope: InstallScope, projectPath: string | null): ActionRequest {
  const copy = COPY[action.type];
  const project = scope === 'project' ? projectPath : null;

  return {
    harness,
    action,
    scope,
    project,
    destructive: copy.destructive ?? false,
    get verb() {
      return copy.verb();
    },
    get title() {
      return copy.title(action);
    },
  };
}

/** The line that says what the command acts on, for the dialog. */
export function scopeSentence(request: ActionRequest, projectName: string | null): string {
  if (request.scope === 'project') {
    return projectName ? t('extensions.scopeLine.projectNamed', { project: projectName }) : t('extensions.scopeLine.projectOpen');
  }

  return t('extensions.scopeLine.user');
}

/** The line of a result that reports a failure, or null. OMP reports failures with exit code 0. */
export function failureLine(result: CliResult): string | null {
  if (!result.ok) {
    return result.summary ?? t('extensions.failure.default');
  }

  const lines = [...result.stderr.split('\n'), ...result.stdout.split('\n')].map((line) => line.trim());

  return lines.find((line) => line.startsWith('✘') || line.includes('Failed to')) ?? null;
}

/** True when the CLI could not preview the action and nothing ran. A config edit has no exit code either, but it does preview. */
export function hasNoPreview(result: CliResult): boolean {
  return result.exitCode === null && !result.timedOut && !result.configEdit;
}

/** OpenCode plugins can be installed from npm and removed, and nothing else: no scope, enable, disable, upgrade, marketplace or package. */
export function installAndRemoveOnly(harness: HarnessId | undefined): boolean {
  return harness === 'opencode';
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
  get confirmed() {
    return t('extensions.outcome.confirmed');
  },
  get missing() {
    return t('extensions.outcome.missing');
  },
  get unchecked() {
    return t('extensions.outcome.unchecked');
  },
};

const NPM_NAME = /^(?:@[A-Za-z0-9][\w.-]*\/)?[A-Za-z0-9][\w.-]*$/;
const NPM_VERSION = /^=?[0-9A-Za-z][\w.+-]*$/;
const NPM_NAME_MAX = 214;
const ARCHIVE = /\.(?:tgz|tar|tar\.gz|zip)$/i;

/** True for `name` or `@scope/name`, optionally followed by `@version` or `@tag`: what the registry resolves, not a path, URL or git address. */
function isNpmRegistrySpec(spec: string): boolean {
  const at = spec.startsWith('@') ? spec.indexOf('@', 1) : spec.indexOf('@');
  const name = at === -1 ? spec : spec.slice(0, at);
  const wanted = at === -1 ? null : spec.slice(at + 1);

  if (name.length > NPM_NAME_MAX || !NPM_NAME.test(name) || ARCHIVE.test(name)) {
    return false;
  }

  return wanted === null || (NPM_VERSION.test(wanted) && !ARCHIVE.test(wanted));
}

/**
 * Mirrors the backend's argument rules, so a bad value is caught before a round trip. `npm` adds the rule of an
 * OpenCode plugin, which must be a package of the npm registry.
 */
export function targetProblem(value: string, kind: 'any' | 'npm' = 'any'): string | null {
  const text = value.trim();

  if (text === '') {
    return t('extensions.problem.empty');
  }
  if (text.length > 512) {
    return t('extensions.problem.tooLong');
  }
  if (text.startsWith('-')) {
    return t('extensions.problem.dash');
  }
  if (!/^[\w@./:+=,-]+$/.test(text)) {
    return t('extensions.problem.characters');
  }
  if (/:\/\/[^/]*@/.test(text)) {
    return t('extensions.problem.credentials');
  }
  if (kind === 'npm' && !isNpmRegistrySpec(text)) {
    return t('extensions.problem.npmSpec');
  }

  return null;
}

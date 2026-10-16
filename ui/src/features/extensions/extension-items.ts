import { t, tn } from '$lib/i18n/i18n.svelte';
import type { AgentInfo } from '$lib/ipc/bindings/AgentInfo';
import type { AgentMode } from '$lib/ipc/bindings/AgentMode';
import type { Capabilities } from '$lib/ipc/bindings/Capabilities';
import type { ExtensionScope } from '$lib/ipc/bindings/ExtensionScope';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { MarketplaceInfo } from '$lib/ipc/bindings/MarketplaceInfo';
import type { McpServerInfo } from '$lib/ipc/bindings/McpServerInfo';
import type { PackageInfo } from '$lib/ipc/bindings/PackageInfo';
import type { PluginInfo } from '$lib/ipc/bindings/PluginInfo';
import type { PromptTemplateInfo } from '$lib/ipc/bindings/PromptTemplateInfo';
import type { SkillInfo } from '$lib/ipc/bindings/SkillInfo';
import type { ThemeInfo } from '$lib/ipc/bindings/ThemeInfo';
import type { ExtensionModuleInfo } from '$lib/ipc/bindings/ExtensionModuleInfo';

export type ExtTab = 'plugins' | 'marketplaces' | 'skills' | 'prompts' | 'agents' | 'mcp' | 'themes' | 'packages' | 'modules';

export interface TabDef {
  id: ExtTab;
  /** The group's name, plural: "Plugins". */
  readonly label: string;
  /** One item of the group: "Plugin", "MCP server". */
  readonly singular: string;
  capability: keyof Capabilities;
}

/** One kind of extension; its texts follow the language on screen, so they are getters over the `extensions.tab.<id>` keys. */
function tab(id: ExtTab, capability: keyof Capabilities): TabDef {
  const key = `extensions.tab.${id}` as const;

  return {
    id,
    get label() {
      return t(key);
    },
    get singular() {
      return t(`${key}.singular`);
    },
    capability,
  };
}

/** All kinds in display order; a harness shows the ones its capabilities enable. */
/**
 * The order of the inventory. What a harness installs comes first (OMP's plugins, PI's packages fill the same role), the
 * kinds most harnesses share follow in one order, and OMP-only catalogs close it, so switching harness keeps Skills,
 * MCP servers and Themes in the same place.
 */
export const TABS: readonly TabDef[] = [
  tab('plugins', 'plugins'),
  tab('packages', 'packages'),
  tab('skills', 'skills'),
  tab('prompts', 'promptTemplates'),
  tab('agents', 'agents'),
  tab('mcp', 'mcp'),
  tab('themes', 'themes'),
  tab('modules', 'extensionModules'),
  tab('marketplaces', 'marketplaces'),
];

export function visibleTabs(capabilities: Capabilities): TabDef[] {
  return TABS.filter((tab) => capabilities[tab.capability]);
}

export type BadgeTone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';

export interface ExtBadge {
  label: string;
  tone: BadgeTone;
}

interface ItemBase {
  id: string;
  title: string;
  subtitle: string | null;
  badges: ExtBadge[];
  /** Lower-cased text the list filter searches. */
  search: string;
  /** A newer version of it (or of something it lists) is available. */
  update: boolean;
}

/** One list row plus the record its detail pane shows. */
export type ExtItem =
  | (ItemBase & { tab: 'plugins'; data: PluginInfo })
  | (ItemBase & { tab: 'marketplaces'; data: MarketplaceInfo; /** Plugins installed from it that have an update, by name. */ updates: Record<string, PluginInfo> })
  | (ItemBase & { tab: 'skills'; data: SkillInfo })
  | (ItemBase & { tab: 'prompts'; data: PromptTemplateInfo })
  | (ItemBase & { tab: 'agents'; data: AgentInfo })
  | (ItemBase & { tab: 'mcp'; data: McpServerInfo })
  | (ItemBase & { tab: 'themes'; data: ThemeInfo })
  | (ItemBase & { tab: 'packages'; data: PackageInfo })
  | (ItemBase & { tab: 'modules'; data: ExtensionModuleInfo });

export function scopeLabel(scope: ExtensionScope): string {
  return t(`extensions.scope.${scope}`);
}

function scopeBadge(scope: ExtensionScope): ExtBadge {
  return { label: scopeLabel(scope), tone: scope === 'project' ? 'info' : 'neutral' };
}

function enabledBadge(enabled: boolean): ExtBadge {
  return enabled ? { label: t('extensions.state.enabled'), tone: 'success' } : { label: t('extensions.state.disabled'), tone: 'neutral' };
}

export function modeLabel(mode: AgentMode): string {
  return t(`extensions.mode.${mode}`);
}

/** Built-in agents say so where a user-defined one names its scope. */
export function agentScope(agent: AgentInfo): ExtensionScope {
  return agent.builtIn ? 'builtin' : agent.scope;
}

function base(id: string, title: string, subtitle: string | null, badges: ExtBadge[], extra: string[] = []): ItemBase {
  const search = [title, subtitle ?? '', ...badges.map((badge) => badge.label), ...extra].join(' ').toLowerCase();

  return { id, title, subtitle, badges, search, update: false };
}

function pluginItems(view: ExtensionsView): ExtItem[] {
  return view.plugins.map((plugin) => {
    const badges: ExtBadge[] = [enabledBadge(plugin.enabled), scopeBadge(plugin.scope)];
    if (plugin.version) {
      badges.push({ label: `v${plugin.version}`, tone: 'neutral' });
    }
    if (plugin.marketplace) {
      badges.push({ label: plugin.marketplace, tone: 'accent' });
    }
    if (plugin.updateAvailable) {
      badges.push({
        label: plugin.latestVersion ? t('extensions.badge.updateTo', { version: plugin.latestVersion }) : t('extensions.badge.updateAvailable'),
        tone: 'info',
      });
    }

    return { ...base(plugin.id, plugin.name, plugin.description, badges, [plugin.id]), update: plugin.updateAvailable, tab: 'plugins', data: plugin };
  });
}

function marketplaceItems(view: ExtensionsView): ExtItem[] {
  return view.marketplaces.map((marketplace) => {
    const badges: ExtBadge[] = [{ label: marketplace.sourceType, tone: 'neutral' }];
    badges.push(
      marketplace.error
        ? { label: t('extensions.badge.unreadable'), tone: 'danger' }
        : { label: tn('extensions.count.plugin', marketplace.entries.length), tone: 'accent' },
    );

    // The catalog's entries are searchable from the one search field, so a marketplace is found by what it lists.
    const listed = marketplace.entries.flatMap((entry) => [entry.name, entry.description ?? '']);

    // The plugins installed from it that are outdated, so the catalog can say so before its own copy is refreshed.
    const updates = Object.fromEntries(
      view.plugins.filter((plugin) => (plugin.marketplace ?? plugin.id.split('@')[1]) === marketplace.name && plugin.updateAvailable).map((plugin) => [plugin.id.split('@')[0], plugin]),
    );

    return { ...base(marketplace.name, marketplace.name, marketplace.source, badges, listed), update: Object.keys(updates).length > 0, tab: 'marketplaces', data: marketplace, updates };
  });
}

function skillItems(view: ExtensionsView): ExtItem[] {
  return view.skills.map((skill) => ({
    ...base(skill.path, skill.name, skill.description, [scopeBadge(skill.scope)], [skill.origin]),
    tab: 'skills',
    data: skill,
  }));
}

function promptItems(view: ExtensionsView): ExtItem[] {
  return view.promptTemplates.map((prompt) => {
    const badges: ExtBadge[] = [scopeBadge(prompt.scope)];
    if (prompt.argumentHint) {
      badges.push({ label: prompt.argumentHint, tone: 'neutral' });
    }

    return { ...base(prompt.path, `/${prompt.name}`, prompt.description, badges), tab: 'prompts', data: prompt };
  });
}

function agentItems(view: ExtensionsView): ExtItem[] {
  return view.agents.map((agent) => {
    const badges: ExtBadge[] = [{ label: modeLabel(agent.mode), tone: agent.mode === 'subagent' ? 'neutral' : 'accent' }, scopeBadge(agentScope(agent))];
    if (agent.hidden) {
      badges.push({ label: t('extensions.state.hidden'), tone: 'warning' });
    }

    return { ...base(`${agent.name}\u001f${agent.path ?? agent.scope}`, agent.name, agent.description, badges, agent.model ? [agent.model] : []), tab: 'agents', data: agent };
  });
}

/** A server's one-line summary: its command, or its URL when that is plain text (never a hidden value). */
function serverSummary(server: McpServerInfo): string | null {
  if (server.command) {
    return [server.command, ...server.args.flatMap((arg) => (arg.kind === 'text' ? [arg.value] : []))].join(' ');
  }

  return server.url?.kind === 'text' ? server.url.value : null;
}

function mcpItems(view: ExtensionsView): ExtItem[] {
  return view.mcpServers.map((server) => {
    const badges: ExtBadge[] = [
      enabledBadge(server.enabled),
      { label: server.transport, tone: 'neutral' },
      { label: scopeLabel(server.source.layer === 'project' ? 'project' : 'user'), tone: server.source.layer === 'project' ? 'info' : 'neutral' },
    ];

    return { ...base(`${server.name}\u001f${server.source.path}`, server.name, serverSummary(server), badges), tab: 'mcp', data: server };
  });
}

function themeItems(view: ExtensionsView): ExtItem[] {
  return view.themes.map((theme) => {
    const badges: ExtBadge[] = [scopeBadge(theme.scope)];
    if (theme.active) {
      badges.unshift({ label: t('extensions.badge.active'), tone: 'accent' });
    }

    return { ...base(`${theme.name}\u001f${theme.scope}`, theme.name, null, badges), tab: 'themes', data: theme };
  });
}

function packageItems(view: ExtensionsView): ExtItem[] {
  return view.packages.map((pkg, index) => {
    const title = pkg.name || (pkg.source.kind === 'text' ? pkg.source.value : t('extensions.badge.hiddenSource'));
    const badges: ExtBadge[] = [{ label: pkg.kind, tone: 'neutral' }, scopeBadge(pkg.scope)];

    return { ...base(`${index}\u001f${title}`, title, null, badges), tab: 'packages', data: pkg };
  });
}

function moduleItems(view: ExtensionsView): ExtItem[] {
  return view.extensionModules.map((module) => ({
    ...base(module.path, module.name, module.path, [enabledBadge(module.enabled), scopeBadge(module.scope)]),
    tab: 'modules',
    data: module,
  }));
}

const BUILDERS: Record<ExtTab, (view: ExtensionsView) => ExtItem[]> = {
  plugins: pluginItems,
  marketplaces: marketplaceItems,
  skills: skillItems,
  prompts: promptItems,
  agents: agentItems,
  mcp: mcpItems,
  themes: themeItems,
  packages: packageItems,
  modules: moduleItems,
};

export function buildItems(tab: ExtTab, view: ExtensionsView): ExtItem[] {
  return BUILDERS[tab](view);
}

/** What a list row shows besides its title: a state dot, a short remark on the right, a pending update. */
export interface RowInfo {
  status: 'on' | 'off' | null;
  meta: string | null;
  update: boolean;
}

export function rowInfo(entry: ExtItem): RowInfo {
  const state = (enabled: boolean) => (enabled ? 'on' : 'off');

  switch (entry.tab) {
    case 'plugins':
      return { status: state(entry.data.enabled), meta: entry.data.version ? `v${entry.data.version}` : null, update: entry.update };
    case 'marketplaces':
      return { status: null, meta: entry.data.error ? t('extensions.badge.unreadable') : entry.data.sourceType, update: entry.update };
    case 'skills':
    case 'prompts':
    case 'themes':
      return { status: null, meta: entry.tab === 'themes' && entry.data.active ? t('extensions.badge.active') : scopeLabel(entry.data.scope), update: false };
    case 'agents':
      return { status: null, meta: modeLabel(entry.data.mode), update: false };
    case 'mcp':
      return { status: state(entry.data.enabled), meta: entry.data.transport, update: false };
    case 'packages':
      return { status: state(entry.data.installed), meta: entry.data.kind, update: false };
    case 'modules':
      return { status: state(entry.data.enabled), meta: scopeLabel(entry.data.scope), update: false };
  }
}

export function filterItems(items: ExtItem[], query: string): ExtItem[] {
  const needle = query.trim().toLowerCase();

  return needle === '' ? items : items.filter((entry) => entry.search.includes(needle));
}

/** The list row of the PI package that provides a skill, when the view has exactly one that fits. Scope tells apart a user and a project package of the same name. */
export function packageOf(view: ExtensionsView, skill: SkillInfo): ExtItem | undefined {
  if (!skill.package) {
    return undefined;
  }

  const matches = buildItems('packages', view).filter((entry) => entry.tab === 'packages' && entry.data.name === skill.package && entry.data.scope === skill.scope);

  // Two packages with one name (acme/tools and evil/tools) leave the skill's owner unknown: offering either one's Uninstall could remove the wrong package.
  return matches.length === 1 ? matches[0] : undefined;
}

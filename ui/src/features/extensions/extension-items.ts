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

export type ExtTab = 'plugins' | 'marketplaces' | 'skills' | 'prompts' | 'mcp' | 'themes' | 'packages' | 'modules';

export interface TabDef {
  id: ExtTab;
  label: string;
  /** Singular/plural noun for empty states. */
  noun: string;
  capability: keyof Capabilities;
}

/** All tabs in display order; a harness shows the ones its capabilities enable. */
export const TABS: readonly TabDef[] = [
  { id: 'plugins', label: 'Plugins', noun: 'plugins', capability: 'plugins' },
  { id: 'marketplaces', label: 'Marketplaces', noun: 'marketplaces', capability: 'marketplaces' },
  { id: 'skills', label: 'Skills', noun: 'skills', capability: 'skills' },
  { id: 'prompts', label: 'Prompts', noun: 'prompt templates', capability: 'promptTemplates' },
  { id: 'mcp', label: 'MCP servers', noun: 'MCP servers', capability: 'mcp' },
  { id: 'themes', label: 'Themes', noun: 'themes', capability: 'themes' },
  { id: 'packages', label: 'Packages', noun: 'packages', capability: 'packages' },
  { id: 'modules', label: 'Modules', noun: 'extension modules', capability: 'extensionModules' },
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
}

/** One list row plus the record its detail pane shows. */
export type ExtItem =
  | (ItemBase & { tab: 'plugins'; data: PluginInfo })
  | (ItemBase & { tab: 'marketplaces'; data: MarketplaceInfo })
  | (ItemBase & { tab: 'skills'; data: SkillInfo })
  | (ItemBase & { tab: 'prompts'; data: PromptTemplateInfo })
  | (ItemBase & { tab: 'mcp'; data: McpServerInfo })
  | (ItemBase & { tab: 'themes'; data: ThemeInfo })
  | (ItemBase & { tab: 'packages'; data: PackageInfo })
  | (ItemBase & { tab: 'modules'; data: ExtensionModuleInfo });

const SCOPE_LABELS: Record<ExtensionScope, string> = {
  user: 'User',
  project: 'Project',
  builtin: 'Built-in',
  plugin: 'Plugin',
};

export function scopeLabel(scope: ExtensionScope): string {
  return SCOPE_LABELS[scope];
}

function scopeBadge(scope: ExtensionScope): ExtBadge {
  return { label: scopeLabel(scope), tone: scope === 'project' ? 'info' : 'neutral' };
}

function enabledBadge(enabled: boolean): ExtBadge {
  return enabled ? { label: 'Enabled', tone: 'success' } : { label: 'Disabled', tone: 'neutral' };
}

function base(id: string, title: string, subtitle: string | null, badges: ExtBadge[], extra: string[] = []): ItemBase {
  const search = [title, subtitle ?? '', ...badges.map((badge) => badge.label), ...extra].join(' ').toLowerCase();

  return { id, title, subtitle, badges, search };
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

    return { ...base(plugin.id, plugin.name, plugin.description, badges, [plugin.id]), tab: 'plugins', data: plugin };
  });
}

function marketplaceItems(view: ExtensionsView): ExtItem[] {
  return view.marketplaces.map((marketplace) => {
    const badges: ExtBadge[] = [{ label: marketplace.sourceType, tone: 'neutral' }];
    badges.push(marketplace.error ? { label: 'Unreadable', tone: 'danger' } : { label: `${marketplace.entries.length} plugins`, tone: 'accent' });

    return { ...base(marketplace.name, marketplace.name, marketplace.source, badges), tab: 'marketplaces', data: marketplace };
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
      { label: server.source.layer === 'project' ? 'Project' : 'User', tone: server.source.layer === 'project' ? 'info' : 'neutral' },
    ];

    return { ...base(`${server.name}\u001f${server.source.path}`, server.name, serverSummary(server), badges), tab: 'mcp', data: server };
  });
}

function themeItems(view: ExtensionsView): ExtItem[] {
  return view.themes.map((theme) => {
    const badges: ExtBadge[] = [scopeBadge(theme.scope)];
    if (theme.active) {
      badges.unshift({ label: 'Active', tone: 'accent' });
    }

    return { ...base(`${theme.name}\u001f${theme.scope}`, theme.name, null, badges), tab: 'themes', data: theme };
  });
}

function packageItems(view: ExtensionsView): ExtItem[] {
  return view.packages.map((pkg, index) => {
    const title = pkg.source.kind === 'text' ? pkg.source.value : 'Hidden source';
    const badges: ExtBadge[] = [{ label: pkg.kind, tone: 'neutral' }, scopeBadge(pkg.scope)];

    return { ...base(`${index}\u001f${title}`, title, pkg.resources.join(', ') || null, badges), tab: 'packages', data: pkg };
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
  mcp: mcpItems,
  themes: themeItems,
  packages: packageItems,
  modules: moduleItems,
};

export function buildItems(tab: ExtTab, view: ExtensionsView): ExtItem[] {
  return BUILDERS[tab](view);
}

export function tabCount(tab: ExtTab, view: ExtensionsView): number {
  return buildItems(tab, view).length;
}

export function filterItems(items: ExtItem[], query: string): ExtItem[] {
  const needle = query.trim().toLowerCase();

  return needle === '' ? items : items.filter((entry) => entry.search.includes(needle));
}

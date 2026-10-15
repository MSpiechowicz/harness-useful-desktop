import type { SyncExcluded } from '../bindings/SyncExcluded';
import type { SyncExportSection } from '../bindings/SyncExportSection';
import type { SyncImportItem } from '../bindings/SyncImportItem';
import type { SyncImportSection } from '../bindings/SyncImportSection';
import type { SyncUnsupported } from '../bindings/SyncUnsupported';

const OMP_MCP = '~/.omp/agent/mcp.json';
const PI_MCP = '~/.pi/agent/mcp.json';
const SKILLS_SOURCE = 'MSpiechowicz/harness-useful-skills';

/** What an import item has in common whatever it is; each fixture overrides what differs. */
function item(partial: Partial<SyncImportItem> & Pick<SyncImportItem, 'id' | 'kind' | 'label'>): SyncImportItem {
  return {
    detail: null,
    version: null,
    status: { type: 'new' },
    selectable: true,
    selectedByDefault: true,
    steps: [],
    runsProgram: false,
    envKeys: [],
    headerKeys: [],
    notes: [],
    dependsOn: null,
    ...partial,
  };
}

const alreadyThere = { status: { type: 'alreadyPresent' }, selectable: false, selectedByDefault: false } as const;

// ── Export ─────────────────────────────────────────────────────────────────

export const EXPORT_SECTIONS: SyncExportSection[] = [
  {
    harness: 'omp',
    items: [
      { id: 'omp:marketplace:harness-useful-skills', kind: 'marketplace', label: 'harness-useful-skills', detail: SKILLS_SOURCE, version: null, runsProgram: false, envKeys: [], headerKeys: [], notes: [] },
      { id: 'omp:marketplace:acme-tools', kind: 'marketplace', label: 'acme-tools', detail: 'acme/omp-tools', version: null, runsProgram: false, envKeys: [], headerKeys: [], notes: [] },
      { id: 'omp:plugin:harness-useful-skills@harness-useful-skills', kind: 'plugin', label: 'harness-useful-skills@harness-useful-skills', detail: null, version: '0.2.36', runsProgram: false, envKeys: [], headerKeys: [], notes: ['versionInformational'] },
      { id: 'omp:plugin:review-helper@acme-tools', kind: 'plugin', label: 'review-helper@acme-tools', detail: null, version: '1.4.0', runsProgram: false, envKeys: [], headerKeys: [], notes: ['versionInformational'] },
      { id: 'omp:mcp:docs-search', kind: 'mcpServer', label: 'docs-search', detail: 'https://docs.example.com/mcp', version: null, runsProgram: false, envKeys: [], headerKeys: ['Authorization'], notes: ['headerValuesNotShared'] },
      { id: 'omp:mcp:local-files', kind: 'mcpServer', label: 'local-files', detail: 'npx -y @acme/files-mcp --root docs', version: null, runsProgram: true, envKeys: ['GITHUB_TOKEN'], headerKeys: [], notes: ['envValuesNotShared', 'writtenDisabled'] },
    ],
  },
  {
    harness: 'pi',
    items: [
      { id: 'pi:package:npm:@acme/pi-theme@1.2.0', kind: 'package', label: 'npm:@acme/pi-theme@1.2.0', detail: null, version: '1.2.0', runsProgram: false, envKeys: [], headerKeys: [], notes: [] },
      { id: 'pi:package:npm:pi-extra', kind: 'package', label: 'npm:pi-extra', detail: null, version: null, runsProgram: false, envKeys: [], headerKeys: [], notes: [] },
      { id: 'pi:mcp:time', kind: 'mcpServer', label: 'time', detail: 'uvx mcp-server-time', version: null, runsProgram: true, envKeys: [], headerKeys: [], notes: ['otherFieldsNotShared'] },
    ],
  },
];

export const EXCLUDED_TYPICAL: SyncExcluded[] = [
  { harness: 'omp', kind: 'mcpServer', label: 'github', reason: 'containsSecret' },
  { harness: 'omp', kind: 'marketplace', label: 'local-dev', reason: 'localPath' },
  { harness: 'pi', kind: 'package', label: 'unknown', reason: 'localPath' },
];

export const EXCLUDED_ALL: SyncExcluded[] = [
  { harness: 'omp', kind: 'mcpServer', label: 'github', reason: 'containsSecret' },
  { harness: 'omp', kind: 'marketplace', label: 'private-market', reason: 'credentials' },
  { harness: 'omp', kind: 'marketplace', label: 'local-dev', reason: 'localPath' },
  { harness: 'omp', kind: 'plugin', label: 'project-only@acme-tools', reason: 'projectScope' },
  { harness: 'pi', kind: 'package', label: 'unknown', reason: 'unsupported' },
  { harness: 'pi', kind: 'mcpServer', label: 'unknown', reason: 'invalid' },
  { harness: 'pi', kind: 'package', label: 'many-more', reason: 'tooMany' },
];

// ── Import ─────────────────────────────────────────────────────────────────

const TYPICAL_OMP: SyncImportSection = {
  harness: 'omp',
  installed: true,
  items: [
    item({
      id: 'omp:marketplace:harness-useful-skills',
      kind: 'marketplace',
      label: 'harness-useful-skills',
      detail: SKILLS_SOURCE,
      steps: [{ type: 'cli', command: ['omp', 'plugin', 'marketplace', 'add', SKILLS_SOURCE] }],
    }),
    item({ id: 'omp:marketplace:acme-tools', kind: 'marketplace', label: 'acme-tools', detail: 'acme/omp-tools', ...alreadyThere }),
    item({
      id: 'omp:plugin:harness-useful-skills@harness-useful-skills',
      kind: 'plugin',
      label: 'harness-useful-skills@harness-useful-skills',
      version: '0.2.36',
      dependsOn: 'omp:marketplace:harness-useful-skills',
      notes: ['versionInformational', 'needsSharedMarketplace'],
      steps: [{ type: 'cli', command: ['omp', 'plugin', 'install', 'harness-useful-skills@harness-useful-skills', '--scope', 'user'] }],
    }),
    item({ id: 'omp:plugin:review-helper@acme-tools', kind: 'plugin', label: 'review-helper@acme-tools', version: '1.4.0', status: { type: 'differentVersion', installed: '1.2.0', shared: '1.4.0' }, selectable: false, selectedByDefault: false }),
    item({
      id: 'omp:plugin:orphan@unknown-market',
      kind: 'plugin',
      label: 'orphan@unknown-market',
      status: { type: 'blocked', reason: 'marketplaceMissing' },
      selectable: false,
      selectedByDefault: false,
    }),
    item({
      id: 'omp:mcp:docs-search',
      kind: 'mcpServer',
      label: 'docs-search',
      headerKeys: ['Authorization'],
      notes: ['headerValuesNotShared', 'writtenDisabled'],
      steps: [{ type: 'mcpMerge', file: OMP_MCP, server: { name: 'docs-search', transport: 'http', command: null, args: [], url: 'https://docs.example.com/mcp', enabled: false } }],
    }),
    item({
      id: 'omp:mcp:local-files',
      kind: 'mcpServer',
      label: 'local-files',
      detail: 'npx -y @acme/files-mcp --root docs',
      selectedByDefault: false,
      runsProgram: true,
      envKeys: ['GITHUB_TOKEN'],
      notes: ['envValuesNotShared', 'writtenDisabled'],
      steps: [{ type: 'mcpMerge', file: OMP_MCP, server: { name: 'local-files', transport: 'stdio', command: 'npx', args: ['-y', '@acme/files-mcp', '--root', 'docs'], url: null, enabled: false } }],
    }),
    item({ id: 'omp:mcp:github', kind: 'mcpServer', label: 'github', status: { type: 'conflict' }, selectable: false, selectedByDefault: false }),
  ],
};

const TYPICAL_PI: SyncImportSection = {
  harness: 'pi',
  installed: true,
  items: [
    item({
      id: 'pi:package:npm:@acme/pi-theme@1.2.0',
      kind: 'package',
      label: 'npm:@acme/pi-theme@1.2.0',
      version: '1.2.0',
      steps: [{ type: 'cli', command: ['pi', 'install', 'npm:@acme/pi-theme@1.2.0'] }],
    }),
    item({ id: 'pi:package:npm:pi-extra', kind: 'package', label: 'npm:pi-extra', ...alreadyThere }),
    item({
      id: 'pi:mcp:time',
      kind: 'mcpServer',
      label: 'time',
      selectedByDefault: false,
      runsProgram: true,
      notes: ['otherFieldsNotShared'],
      steps: [{ type: 'mcpMerge', file: PI_MCP, server: { name: 'time', transport: 'stdio', command: 'uvx', args: ['mcp-server-time'], url: null, enabled: true } }],
    }),
  ],
};

export const TYPICAL_PLAN_SECTIONS: SyncImportSection[] = [TYPICAL_OMP, TYPICAL_PI];

/** Hostile input: items the backend refused, and a harness that is not installed here. */
export const HOSTILE_PLAN_SECTIONS: SyncImportSection[] = [
  {
    harness: 'omp',
    installed: true,
    items: [
      item({ id: 'omp:marketplace:#1', kind: 'marketplace', label: '#1', status: { type: 'invalid', reason: 'unsafeCharacters' }, selectable: false, selectedByDefault: false }),
      item({ id: 'omp:plugin:#2', kind: 'plugin', label: '#2', status: { type: 'invalid', reason: 'unsupportedSource' }, selectable: false, selectedByDefault: false }),
      item({ id: 'omp:mcp:#3', kind: 'mcpServer', label: '#3', status: { type: 'invalid', reason: 'shellCommand' }, selectable: false, selectedByDefault: false }),
      item({ id: 'omp:mcp:#4', kind: 'mcpServer', label: '#4', status: { type: 'invalid', reason: 'containsSecret' }, selectable: false, selectedByDefault: false }),
      item({ id: 'omp:marketplace:#5', kind: 'marketplace', label: '#5', status: { type: 'invalid', reason: 'credentials' }, selectable: false, selectedByDefault: false }),
    ],
  },
  {
    harness: 'pi',
    installed: false,
    items: [
      item({ id: 'pi:package:npm:pi-extra', kind: 'package', label: 'npm:pi-extra', status: { type: 'blocked', reason: 'harnessNotInstalled' }, selectable: false, selectedByDefault: false }),
      item({ id: 'pi:mcp:time', kind: 'mcpServer', label: 'time', status: { type: 'blocked', reason: 'harnessNotInstalled' }, selectable: false, selectedByDefault: false }),
    ],
  },
];

export const HOSTILE_UNSUPPORTED: SyncUnsupported[] = [
  { harness: null, name: 'zed', itemCount: 4 },
  { harness: null, name: 'unknown', itemCount: 2 },
  { harness: 'omp', name: 'themes', itemCount: 3 },
];

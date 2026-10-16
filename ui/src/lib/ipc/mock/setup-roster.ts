import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { McpState } from '../bindings/McpState';
import type { SkillsSetup } from '../bindings/SkillsSetup';

/**
 * The harnesses the recommended setup is meant to scale to: the preview's `?setup=many` shows five of them, and
 * `?setup=manyLong` adds a sixth with a very long name. Only the browser preview knows these ids; the real `HarnessId`
 * is the backend's. They are chosen to cover every state a row can be in.
 */
export interface RosterEntry {
  id: HarnessId;
  name: string;
  shortName: string;
  installed: boolean;
  skills: SkillsSetup;
  /** The dashboard's MCP state before anything is set up. */
  mcp: McpState;
  /** The memory server's MCP state before anything is set up. */
  memoryMcp: McpState;
}

const asId = (id: string): HarnessId => id as HarnessId;

const SKILLS_ACTIONS: SkillsSetup = {
  type: 'install',
  actions: [
    { type: 'marketplaceAdd', source: 'MSpiechowicz/harness-useful-skills' },
    { type: 'pluginInstall', target: 'harness-useful-skills@harness-useful-skills', scope: 'user' },
  ],
};

const FIVE: RosterEntry[] = [
  {
    id: asId('omp'),
    name: 'Oh My Pi',
    shortName: 'OMP',
    installed: true,
    skills: SKILLS_ACTIONS,
    mcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: true },
    memoryMcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: true },
  },
  {
    id: asId('pi'),
    name: 'Pi',
    shortName: 'PI',
    installed: true,
    skills: { type: 'install', actions: [{ type: 'packageInstall', source: 'git:github.com/MSpiechowicz/harness-useful-skills' }] },
    mcp: { type: 'refused', file: '~/.pi/agent/mcp.json', reason: 'unparsable' },
    memoryMcp: { type: 'refused', file: '~/.pi/agent/mcp.json', reason: 'unparsable' },
  },
  {
    id: asId('claude'),
    name: 'Claude Code',
    shortName: 'CC',
    installed: true,
    skills: { type: 'installed', version: '0.2.36' },
    mcp: { type: 'configured', file: '~/.claude.json', name: 'harness-dashboard' },
    memoryMcp: { type: 'configured', file: '~/.claude.json', name: 'harness-memory' },
  },
  {
    id: asId('codex'),
    name: 'Codex',
    shortName: 'CDX',
    installed: true,
    skills: { type: 'unavailable', reason: 'Not supported yet.' },
    mcp: { type: 'disabled', file: '~/.codex/config.toml', name: 'harness-dashboard' },
    memoryMcp: { type: 'missing', file: '~/.codex/config.toml', exists: true },
  },
  {
    id: asId('opencode'),
    name: 'OpenCode',
    shortName: 'OC',
    installed: false,
    skills: SKILLS_ACTIONS,
    mcp: { type: 'missing', file: '~/.config/opencode/opencode.json', exists: false },
    memoryMcp: { type: 'missing', file: '~/.config/opencode/opencode.json', exists: false },
  },
];

const LONG: RosterEntry = {
  id: asId('long'),
  name: 'Some Very Long Harness Name',
  shortName: 'SLH',
  installed: true,
  skills: { type: 'installed', version: null },
  mcp: { type: 'refused', file: '', reason: 'unparsable' },
  memoryMcp: { type: 'refused', file: '', reason: 'unparsable' },
};

/** The roster of a scenario, or null when the scenario uses the real two harnesses. */
export function rosterFor(scenario: string): RosterEntry[] | null {
  if (scenario === 'many') {
    return FIVE;
  }

  return scenario === 'manyLong' ? [...FIVE, LONG] : null;
}

/** The harness list the rest of the app sees in such a scenario: the first harness's summary, reworded per entry. */
export function rosterSummaries(roster: RosterEntry[], template: HarnessSummary): HarnessSummary[] {
  return roster.map((entry) => ({
    ...template,
    info: { ...template.info, id: entry.id, name: entry.name, shortName: entry.shortName, binaryName: entry.id },
    detection: { ...template.detection, installed: entry.installed },
  }));
}

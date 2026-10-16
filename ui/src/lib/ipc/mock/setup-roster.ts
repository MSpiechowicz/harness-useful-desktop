import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { McpState } from '../bindings/McpState';
import type { SkillsSetup } from '../bindings/SkillsSetup';

/**
 * The harnesses the recommended setup is meant to scale to: the preview's `?setup=many` shows five of them, and
 * `?setup=manyLong` adds a sixth with a very long name. Ids the backend does not have (`claude`, `codex`) exist only in
 * the browser preview, the real `HarnessId` is the backend's. They are chosen to cover every state a row can be in.
 * `?setup=opencodeOnly` is a machine with OpenCode alone, whose config file has comments.
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

const OPENCODE_SKILLS: SkillsSetup = { type: 'unavailable', reason: 'Harness Skills has no OpenCode package yet.' };

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
    shortName: 'OpenCode',
    installed: false,
    skills: OPENCODE_SKILLS,
    mcp: { type: 'missing', file: '~/.config/opencode/opencode.json', exists: false },
    memoryMcp: { type: 'missing', file: '~/.config/opencode/opencode.json', exists: false },
  },
];

/** OpenCode's config file with comments: the app leaves it alone and shows the entry to paste. */
const OPENCODE_FILE = '~/.config/opencode/opencode.jsonc';

/** The entry a person adds to a commented OpenCode config by hand. */
export function opencodeSnippet(server: string): string {
  return `{
  "mcp": {
    "${server}": {
      "type": "local",
      "command": ["${server}", "mcp"],
      "enabled": true
    }
  }
}`;
}

/** A machine with OpenCode alone: the other harnesses are not installed, and OpenCode's file needs a manual edit. */
const OPENCODE_ONLY: RosterEntry[] = [
  {
    id: asId('omp'),
    name: 'Oh My Pi',
    shortName: 'OMP',
    installed: false,
    skills: SKILLS_ACTIONS,
    mcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: false },
    memoryMcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: false },
  },
  {
    id: asId('pi'),
    name: 'Pi',
    shortName: 'PI',
    installed: false,
    skills: SKILLS_ACTIONS,
    mcp: { type: 'missing', file: '~/.pi/agent/mcp.json', exists: false },
    memoryMcp: { type: 'missing', file: '~/.pi/agent/mcp.json', exists: false },
  },
  {
    id: asId('opencode'),
    name: 'OpenCode',
    shortName: 'OpenCode',
    installed: true,
    skills: OPENCODE_SKILLS,
    mcp: { type: 'manualEdit', file: OPENCODE_FILE, snippet: opencodeSnippet('harness-dashboard') },
    memoryMcp: { type: 'manualEdit', file: OPENCODE_FILE, snippet: opencodeSnippet('harness-memory') },
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

/** The roster of a scenario, or null when the scenario uses the real harnesses. */
export function rosterFor(scenario: string): RosterEntry[] | null {
  if (scenario === 'many') {
    return FIVE;
  }

  if (scenario === 'opencodeOnly') {
    return OPENCODE_ONLY;
  }

  return scenario === 'manyLong' ? [...FIVE, LONG] : null;
}

/**
 * The harness list the rest of the app sees in such a scenario. A harness the backend knows keeps its own summary, with
 * only whether it is installed taken from the entry. One it does not know is the first harness's summary, reworded.
 */
export function rosterSummaries(roster: RosterEntry[], list: HarnessSummary[]): HarnessSummary[] {
  const template = list[0];

  return roster.map((entry) => {
    const known = list.find((summary) => summary.info.id === entry.id);
    const base = known ?? template;

    return {
      ...base,
      info: known ? known.info : { ...template.info, id: entry.id, name: entry.name, shortName: entry.shortName, binaryName: entry.id },
      detection: entry.installed ? { ...base.detection, installed: true } : { ...base.detection, installed: false, binaryPath: null, version: null },
    };
  });
}

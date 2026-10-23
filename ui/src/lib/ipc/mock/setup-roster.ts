import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { McpState } from '../bindings/McpState';
import type { SkillsSetup } from '../bindings/SkillsSetup';

/**
 * The harnesses the recommended setup is meant to scale to: the preview's `?setup=many` shows the five the backend has,
 * and `?setup=manyLong` adds a sixth with a very long name, whose id (`long`) exists only in the browser preview. Their
 * states are chosen to cover every state a row can be in. `?setup=opencodeOnly` is a machine with OpenCode alone, whose
 * config file has comments.
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
  /** The app's own MCP server (hooks for agents) before anything is set up. */
  desktopMcp: McpState;
}

const asId = (id: string): HarnessId => id as HarnessId;

const SKILLS_ACTIONS: SkillsSetup = {
  type: 'install',
  actions: [
    { type: 'marketplaceAdd', source: 'MSpiechowicz/harness-useful-skills' },
    { type: 'pluginInstall', target: 'harness-useful-skills@harness-useful-skills', scope: 'user' },
  ],
};

/** The Harness Skills' own installer for OpenCode and what it changes, as the backend words them. */
export const OPENCODE_SKILLS_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh | sh";
export const OPENCODE_SKILLS_SIDE_EFFECTS = [
  'Installs the Harness Skills plugin, skills, agents and commands into the OpenCode config folder (~/.config/opencode).',
  'Adds the harness-useful-skills MCP server to opencode.json in that folder.',
  'Needs Node.js 22 or newer.',
  'Restart OpenCode and running OpenCode chats to load the skills.',
  'Updates come from /useful-skills update in OpenCode.',
];

/** The same installer for Codex (`HARNESS_SKILLS_HOST=codex`) and what it changes, as the backend words them. */
export const CODEX_SKILLS_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh | HARNESS_SKILLS_HOST=codex sh";
export const CODEX_SKILLS_SIDE_EFFECTS = [
  'Adds the Harness Skills marketplace and plugin to Codex.',
  'Writes the us-* agents and the memory runtime into the Codex folder (~/.codex).',
  'Registers the harness-useful-skills MCP server with codex mcp add.',
  'Needs Node.js 22 or newer and the codex command.',
  'Restart Codex and running Codex chats to load the skills.',
  'Updates come from running this installer again.',
];

/** What `setup_status` reports for OpenCode's skills when Node.js is found. */
export const OPENCODE_SKILLS: SkillsSetup = { type: 'script', command: OPENCODE_SKILLS_COMMAND, sideEffects: OPENCODE_SKILLS_SIDE_EFFECTS, outdatedFrom: null };

/** What it reports for Codex's skills when Node.js is found. */
export const CODEX_SKILLS: SkillsSetup = { type: 'script', command: CODEX_SKILLS_COMMAND, sideEffects: CODEX_SKILLS_SIDE_EFFECTS, outdatedFrom: null };

/** What it reports when Node.js is missing. */
export const SKILLS_NO_NODE: SkillsSetup = { type: 'unavailable', reason: 'Needs Node.js 22 or newer.', code: 'nodeMissing' };

const FIVE: RosterEntry[] = [
  {
    id: asId('omp'),
    name: 'Oh My Pi',
    shortName: 'OMP',
    installed: true,
    skills: SKILLS_ACTIONS,
    mcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: true },
    memoryMcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: true },
    desktopMcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: true },
  },
  {
    id: asId('pi'),
    name: 'Pi',
    shortName: 'PI',
    installed: true,
    skills: { type: 'install', actions: [{ type: 'packageInstall', source: 'git:github.com/MSpiechowicz/harness-useful-skills' }] },
    mcp: { type: 'refused', file: '~/.pi/agent/mcp.json', reason: 'unparsable' },
    memoryMcp: { type: 'refused', file: '~/.pi/agent/mcp.json', reason: 'unparsable' },
    desktopMcp: { type: 'refused', file: '~/.pi/agent/mcp.json', reason: 'unparsable' },
  },
  {
    id: asId('claude'),
    name: 'Claude Code',
    shortName: 'CC',
    installed: true,
    skills: { type: 'installed', version: '0.2.36' },
    mcp: { type: 'configured', file: '~/.claude.json', name: 'harness-dashboard' },
    memoryMcp: { type: 'disabled', file: '~/.claude.json', name: 'harness-memory' },
    desktopMcp: { type: 'configured', file: '~/.claude.json', name: 'harness-desktop' },
  },
  {
    id: asId('codex'),
    name: 'Codex',
    shortName: 'CX',
    installed: true,
    skills: CODEX_SKILLS,
    mcp: { type: 'missing', file: '~/.codex/config.toml', exists: true },
    memoryMcp: { type: 'missing', file: '~/.codex/config.toml', exists: true },
    desktopMcp: { type: 'missing', file: '~/.codex/config.toml', exists: true },
  },
  {
    id: asId('opencode'),
    name: 'OpenCode',
    shortName: 'OpenCode',
    installed: false,
    skills: OPENCODE_SKILLS,
    mcp: { type: 'missing', file: '~/.config/opencode/opencode.json', exists: false },
    memoryMcp: { type: 'missing', file: '~/.config/opencode/opencode.json', exists: false },
    desktopMcp: { type: 'missing', file: '~/.config/opencode/opencode.json', exists: false },
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
    desktopMcp: { type: 'missing', file: '~/.omp/agent/mcp.json', exists: false },
  },
  {
    id: asId('pi'),
    name: 'Pi',
    shortName: 'PI',
    installed: false,
    skills: SKILLS_ACTIONS,
    mcp: { type: 'missing', file: '~/.pi/agent/mcp.json', exists: false },
    memoryMcp: { type: 'missing', file: '~/.pi/agent/mcp.json', exists: false },
    desktopMcp: { type: 'missing', file: '~/.pi/agent/mcp.json', exists: false },
  },
  {
    id: asId('opencode'),
    name: 'OpenCode',
    shortName: 'OpenCode',
    installed: true,
    skills: OPENCODE_SKILLS,
    mcp: { type: 'manualEdit', file: OPENCODE_FILE, snippet: opencodeSnippet('harness-dashboard') },
    memoryMcp: { type: 'manualEdit', file: OPENCODE_FILE, snippet: opencodeSnippet('harness-memory') },
    desktopMcp: { type: 'manualEdit', file: OPENCODE_FILE, snippet: opencodeSnippet('harness-desktop') },
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
  desktopMcp: { type: 'refused', file: '', reason: 'unparsable' },
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

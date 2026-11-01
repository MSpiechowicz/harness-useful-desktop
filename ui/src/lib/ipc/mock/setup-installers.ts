import type { DashboardInstallResult } from '../bindings/DashboardInstallResult';
import type { InstallLogLine } from '../bindings/InstallLogLine';
import { CODEX_SKILLS_COMMAND, OPENCODE_SKILLS_COMMAND } from './setup-roster';

/** Pause between two installer log lines; tests do not wait. */
export const LINE_MS = import.meta.env.MODE === 'test' ? 0 : 160;

export function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** The install command a family member's script is run with, as the backend words it. */
function installScript(repo: string): string {
  return `curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/${repo}/main/install.sh | sh`;
}

export const VERSION = '0.9.4';
export const COMMAND = installScript('harness-useful-dashboard');
export const SIDE_EFFECTS = [
  'Installs the Harness Dashboard app to ~/.local/lib/harness-dashboard and the harness-dashboard command to ~/.local/bin.',
  'Adds Harness Dashboard to your applications menu.',
  'Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.',
  'Stops a running dashboard server or app so the new version can start, and `harness-dashboard mcp` keeps running.',
];

export const MEMORY_VERSION = '1.0.0';
export const MEMORY_COMMAND = installScript('harness-useful-memory');
export const MEMORY_SIDE_EFFECTS = [
  'Installs the harness-memory command to ~/.local/bin, replacing an older one there.',
  'Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.',
  'Running memory servers keep working, and each harness starts the new version next time.',
];

export const KNOWLEDGE_VERSION = '1.0.0';
export const KNOWLEDGE_COMMAND = installScript('harness-useful-knowledge');
export const KNOWLEDGE_SIDE_EFFECTS = [
  'Installs the harness-knowledge command to ~/.local/bin, replacing an older one there.',
  'Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.',
  'Running knowledge servers keep working, and each harness starts the new version next time.',
];

export const OPENCODE_SKILLS_VERSION = '0.2.40';
export const CODEX_SKILLS_VERSION = '0.3.13';

/** The output of a binary's installer: the download, the checksum, the install and the version it ends with. */
function binaryOutput(binary: string, version: string): InstallLogLine[] {
  return [
    { stream: 'stdout', line: `==> Downloading ${binary}-x86_64-unknown-linux-gnu (latest)` },
    { stream: 'stdout', line: '==> Checksum verified' },
    { stream: 'stdout', line: `==> Installed ${binary} to ~/.local/bin/${binary}` },
    { stream: 'stdout', line: `==> ${binary} ${version}` },
  ];
}

function binaryFailed(binary: string): InstallLogLine[] {
  return [
    { stream: 'stdout', line: `==> Downloading ${binary}-x86_64-unknown-linux-gnu (latest)` },
    { stream: 'stderr', line: 'error: download failed' },
  ];
}

/** The output of a skills installer that needs Node.js and ends by telling the user to restart the harness. */
function skillsOutput(steps: string[], version: string, harness: string): InstallLogLine[] {
  return [
    { stream: 'stdout', line: '==> Checking Node.js (22 or newer)' },
    ...steps.map((line): InstallLogLine => ({ stream: 'stdout', line })),
    { stream: 'stdout', line: `==> Harness Skills ${version} installed. Restart ${harness} to load them.` },
  ];
}

const SKILLS_OUTPUT_FAILED: InstallLogLine[] = [
  { stream: 'stdout', line: '==> Checking Node.js (22 or newer)' },
  { stream: 'stderr', line: 'error: could not download the Harness Skills package' },
];

const OUTPUT_OK: InstallLogLine[] = [
  { stream: 'stdout', line: 'Downloading harness-dashboard 0.9.4 for linux-x86_64' },
  { stream: 'stdout', line: 'Verifying checksum… ok' },
  { stream: 'stdout', line: 'Installing to ~/.local/lib/harness-dashboard' },
  { stream: 'stdout', line: 'Linking ~/.local/bin/harness-dashboard' },
  { stream: 'stdout', line: 'Done. Run `harness-dashboard` to start.' },
];

const OUTPUT_FAILED: InstallLogLine[] = [
  { stream: 'stdout', line: 'Downloading harness-dashboard 0.9.4 for linux-x86_64' },
  { stream: 'stderr', line: 'curl: (22) The requested URL returned error: 404' },
  { stream: 'stderr', line: 'Could not download the release archive.' },
];

/** One companion installer of the mock: what it prints and reports. */
export interface MockInstaller {
  binary: string;
  version: string;
  command: string;
  output: InstallLogLine[];
  failedOutput: InstallLogLine[];
  failedSummary: string;
}

/** The installer of a family server (`harness-memory`, `harness-knowledge`). */
function serverInstaller(binary: string, version: string, command: string): MockInstaller {
  return { binary, version, command, output: binaryOutput(binary, version), failedOutput: binaryFailed(binary), failedSummary: 'error: download failed' };
}

export const DASHBOARD_INSTALLER: MockInstaller = {
  binary: 'harness-dashboard',
  version: VERSION,
  command: COMMAND,
  output: OUTPUT_OK,
  failedOutput: OUTPUT_FAILED,
  failedSummary: 'The installer exited with code 22.',
};

export const MEMORY_INSTALLER = serverInstaller('harness-memory', MEMORY_VERSION, MEMORY_COMMAND);

export const KNOWLEDGE_INSTALLER = serverInstaller('harness-knowledge', KNOWLEDGE_VERSION, KNOWLEDGE_COMMAND);

export const OPENCODE_SKILLS_INSTALLER: MockInstaller = {
  binary: 'Harness Skills for OpenCode',
  version: OPENCODE_SKILLS_VERSION,
  command: OPENCODE_SKILLS_COMMAND,
  output: skillsOutput(['==> Installing the Harness Skills into ~/.config/opencode', '==> Adding the harness-useful-skills MCP server to opencode.json'], OPENCODE_SKILLS_VERSION, 'OpenCode'),
  failedOutput: SKILLS_OUTPUT_FAILED,
  failedSummary: 'error: could not download the Harness Skills package',
};

export const CODEX_SKILLS_INSTALLER: MockInstaller = {
  binary: 'Harness Skills for Codex',
  version: CODEX_SKILLS_VERSION,
  command: CODEX_SKILLS_COMMAND,
  output: skillsOutput(['==> Adding the Harness Skills marketplace and plugin to Codex', '==> Registering the harness-useful-skills MCP server'], CODEX_SKILLS_VERSION, 'Codex'),
  failedOutput: SKILLS_OUTPUT_FAILED,
  failedSummary: 'error: could not download the Harness Skills package',
};

/** Plays `installer` line by line, and it fails when `fails`. */
export async function runInstaller(installer: MockInstaller, fails: boolean, onLog: (line: InstallLogLine) => void): Promise<DashboardInstallResult> {
  for (const line of fails ? installer.failedOutput : installer.output) {
    await delay(LINE_MS);
    onLog(line);
  }

  return {
    ok: !fails,
    summary: fails ? installer.failedSummary : `Installed ${installer.binary} ${installer.version}.`,
    exitCode: fails ? 22 : 0,
    timedOut: false,
    truncated: false,
    version: fails ? null : installer.version,
    command: installer.command,
  };
}

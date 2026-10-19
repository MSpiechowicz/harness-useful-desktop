import { ipc } from '$lib/ipc/client';

/** Whether any harness has the app's own MCP server, through which agents write hooks and routines: unknown until the machine was read. */
export type DesktopMcpState = 'unknown' | 'registered' | 'missing';

/** The state of the app's own MCP server in the harnesses, read once for every view that offers a way to add it. */
export class DesktopMcpStore {
  state = $state<DesktopMcpState>('unknown');

  /** Reads whether a harness has the app's MCP server registered. A machine that cannot be read says nothing, rather than nagging. */
  async read(): Promise<void> {
    try {
      const setup = await ipc.invoke('setup_status', undefined);
      const installed = setup.harnesses.filter((entry) => entry.installed);

      this.state = installed.length === 0 || installed.some((entry) => entry.desktopMcp.type === 'configured') ? 'registered' : 'missing';
    } catch {
      this.state = 'unknown';
    }
  }

  reset(): void {
    this.state = 'unknown';
  }
}

export const desktopMcp = new DesktopMcpStore();

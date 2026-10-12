import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import type { Transport } from '$lib/ipc/transport';

/** A `matchMedia` stand-in whose result can be flipped to fire `change` events. */
export function fakeMedia(initial: boolean) {
  const listeners = new Set<(event: MediaQueryListEvent) => void>();
  let matches = initial;

  const list = {
    get matches() {
      return matches;
    },
    addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => listeners.add(listener),
    removeEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => listeners.delete(listener),
  } as unknown as MediaQueryList;

  return {
    source: { matchMedia: () => list } as Pick<Window, 'matchMedia'>,
    set(next: boolean) {
      matches = next;
      listeners.forEach((listener) => listener({ matches: next } as MediaQueryListEvent));
    },
  };
}

export function samplePrefs(overrides: Partial<AppPrefs> = {}): AppPrefs {
  return {
    version: 1,
    theme: 'system',
    motion: 'full',
    introEnabled: true,
    soundEnabled: true,
    showThinking: true,
    defaultHarness: null,
    activeProject: null,
    rememberRecentProjects: true,
    recentProjects: [],
    updatePolicy: 'startupAsk',
    checkCompanionUpdates: true,
    openAnyLink: false,
    language: 'en',
    onboardingCompleted: false,
    approvalNoticeSeen: false,
    binaryOverrides: [],
    chatDefaults: [],
    ...overrides,
  };
}

type Handler = (args: never) => unknown;

/** Transport answering from a per-command handler table; unlisted commands throw. */
export function fakeTransport(handlers: Partial<Record<CommandName, Handler>>): Transport {
  return {
    async invoke<K extends CommandName>(name: K, args: CommandArgs<K>): Promise<CommandResult<K>> {
      const handler = handlers[name];
      if (!handler) {
        throw new Error(`unexpected command ${name}`);
      }

      return (await handler(args as never)) as CommandResult<K>;
    },
  };
}

import { fireEvent, screen } from '@testing-library/svelte';
import { vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';

/**
 * Answers the commands that `handlers` has (a folder of hooks or routines kept in memory) and leaves the rest to the mock
 * transport as usual. `after` may change what the transport answered, e.g. to pretend a machine is set up differently.
 */
export function spyOnInvoke(handlers: object, after?: <K extends CommandName>(name: K, result: CommandResult<K>) => CommandResult<K>) {
  const passthrough = ipc.invoke.bind(ipc);

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    const handler = (handlers as Record<string, (args: unknown) => unknown>)[name];

    if (handler) {
      return (await handler(args)) as CommandResult<K>;
    }

    const result = (await passthrough(name, args)) as CommandResult<K>;

    return after ? after(name, result) : result;
  });
}

/** The arguments of every call of a command. */
export function calls(invoke: { mock: { calls: unknown[][] } }, name: string): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);
}

/** Opens a filter menu by its button and picks one of its choices. */
export async function pick(button: string, choice: RegExp): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: button }));
  await fireEvent.click(await screen.findByRole('menuitemradio', { name: choice }));
}

/** What a documents table lists, top to bottom: the heading of each group ("# Global") followed by the names of its rows. */
export function listed(grid: HTMLElement): string[] {
  return [...grid.querySelectorAll<HTMLElement>('tbody tr')].map((row) => {
    const heading = row.querySelector('th[scope="colgroup"]');

    return heading ? `# ${heading.querySelector('span span')?.textContent?.trim() ?? ''}` : (row.querySelector('td .truncate')?.textContent?.trim() ?? '');
  });
}

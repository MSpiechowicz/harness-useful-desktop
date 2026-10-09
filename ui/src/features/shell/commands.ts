import type { PaletteCommand } from '$lib/components/palette';

import { actionCommands, appearanceCommands, navigationCommands, type PaletteHandlers } from './palette/actions';
import { configCommands, extensionCommands, fileCommands, sessionCommands, slashCommands } from './palette/content';
import { buildSwitchCommands } from './switch-commands';

/**
 * Everything the Ctrl/Cmd+K palette can find: views, actions, and the content of the project
 * (sessions, changed files, configuration keys, extensions, slash commands). Rebuilt reactively
 * because most entries depend on state.
 */
export function buildCommands(handlers: PaletteHandlers): PaletteCommand[] {
  return [
    ...navigationCommands(),
    ...actionCommands(handlers),
    ...buildSwitchCommands(),
    ...appearanceCommands(),
    ...sessionCommands(),
    ...fileCommands(),
    ...configCommands(),
    ...extensionCommands(),
    ...slashCommands(),
  ];
}

import type { CommandArgs, CommandName, CommandResult } from '../commands';

export type Handlers = {
  [K in CommandName]: (args: CommandArgs<K>) => CommandResult<K> | Promise<CommandResult<K>>;
};

/** A named, self-contained backend behaviour for browser previews and e2e runs. */
export interface Scenario {
  name: string;
  handlers: Handlers;
}

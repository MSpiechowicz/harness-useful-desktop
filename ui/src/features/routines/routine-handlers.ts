/** What the panel of a routine can ask for: the view wires each to the store, the panel only calls them. */
export interface RoutineHandlers {
  toggle: (enabled: boolean) => void;
  approve: () => void;
  reject: () => void;
  run: () => void;
  stop: () => void;
  edit: () => void;
  remove: () => void;
  share: () => void;
  update: () => void;
  removeTeam: () => void;
  copy: () => void;
  /** Answers the question of a run that waits for the user: opens its chat. */
  openChat: () => void;
  /** Opens the chat session of the run going on in Sessions. */
  openSession: () => void;
  /** Opens a run of this routine in the run history, or its whole history with no run. */
  showRuns: (runId?: string) => void;
  close: () => void;
}

/** Handlers that do nothing: for a panel that has no routine to work on (it is closed and nothing was ever opened). */
export const NO_HANDLERS: RoutineHandlers = {
  toggle: () => {},
  approve: () => {},
  reject: () => {},
  run: () => {},
  stop: () => {},
  edit: () => {},
  remove: () => {},
  share: () => {},
  update: () => {},
  removeTeam: () => {},
  copy: () => {},
  openChat: () => {},
  openSession: () => {},
  showRuns: () => {},
  close: () => {},
};

import { readStored, writeStored } from '$lib/utils/persisted';

const RAIL_KEY = 'harness.railExpanded';

/** Below this viewport width the rail always stays icon-only so the content keeps its room. */
export const RAIL_EXPAND_MIN_WIDTH = 1100;

/** Layout choices of the shell that survive restarts. */
export class UiState {
  /** The user's choice; whether it applies also depends on the window width (`railShowsLabels`). */
  railExpanded = $state(readStored(RAIL_KEY) === 'true');
  /** Window is wide enough for the expanded rail. */
  roomForLabels = $state(true);

  railShowsLabels: boolean = $derived(this.railExpanded && this.roomForLabels);

  setRailExpanded(value: boolean): void {
    this.railExpanded = value;
    writeStored(RAIL_KEY, String(value));
  }

  toggleRail(): void {
    this.setRailExpanded(!this.railExpanded);
  }
}

export const ui = new UiState();

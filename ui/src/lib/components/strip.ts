import type { IconComponent } from './icon';

export interface StripOption<V extends string = string> {
  value: V;
  label: string;
  icon?: IconComponent;
  disabled?: boolean;
  /** Small count pill after the label. */
  count?: number;
}

/** The frame of a pill group (the Structured / Source switch, the layout toggles): as tall as a button, so they line up. */
export const PILL_FRAME = 'h-control rounded-md border border-line bg-surface-2 p-[var(--segment-pad)] gap-[var(--segment-gap)]';

/** What lifts the chosen (or open) pill out of its frame. */
export const PILL_RAISED = 'rounded-sm border border-line-strong bg-surface-4 shadow-raised';

import type { IconComponent } from './icon';

/** One other way to do what a `SplitButton`'s main action does. */
export interface SplitItem {
  id: string;
  label: string;
  /** A dim second line under the label. */
  description?: string;
  icon?: IconComponent;
  disabled?: boolean;
  onselect: () => void;
}

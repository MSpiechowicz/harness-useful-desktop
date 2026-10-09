import type { IconComponent } from './icon';

export interface StripOption<V extends string = string> {
  value: V;
  label: string;
  icon?: IconComponent;
  disabled?: boolean;
}

import { common } from './common';
import { shell } from './shell';
import { chat } from './chat';
import { sessions } from './sessions';
import { changes } from './changes';
import { config } from './config';
import { extensions } from './extensions';
import { ecosystem } from './ecosystem';
import { onboarding } from './onboarding';
import { settings } from './settings';
import { updates } from './updates';
import { harness } from './harness';
import { components } from './components';
import { sync } from './sync';

export const en = {
  ...common,
  ...shell,
  ...chat,
  ...sessions,
  ...changes,
  ...config,
  ...extensions,
  ...ecosystem,
  ...onboarding,
  ...settings,
  ...updates,
  ...harness,
  ...components,
  ...sync,
} satisfies Record<string, string>;

/** Every message key of the app; the other languages must have exactly these. */
export type MessageKey = keyof typeof en;

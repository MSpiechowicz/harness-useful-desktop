import type { Dict } from '../types';
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

export const fr: Dict = {
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
};

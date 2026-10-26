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
import { sync } from './sync';
import { memory } from './memory';
import { hooks } from './hooks';
import { routines } from './routines';
import { memoryEditor } from './memory-editor';
import { memoryImport } from './memory-import';
import { memoryGuards } from './memory-guards';
import { branch } from './branch';

export const pl: Dict = {
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
  ...memory,
  ...hooks,
  ...routines,
  ...memoryEditor,
  ...memoryImport,
  ...memoryGuards,
  ...branch,
};

import type { CommandSpec } from '../bindings/CommandSpec';
import type { ConfigView } from '../bindings/ConfigView';
import type { ExtensionsView } from '../bindings/ExtensionsView';
import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { ProjectInfo } from '../bindings/ProjectInfo';
import commandsOmp from './fixtures/commands_catalog.omp.json';
import commandsPi from './fixtures/commands_catalog.pi.json';
import configOmp from './fixtures/config_view.omp.json';
import configPi from './fixtures/config_view.pi.json';
import extensionsOmp from './fixtures/extensions_view.omp.json';
import extensionsPi from './fixtures/extensions_view.pi.json';
import harnessList from './fixtures/harness_list.json';
import projectRecent from './fixtures/project_recent.json';

/**
 * Backend-generated sample data (see `business`); the mock transport serves these
 * untouched so the browser preview shows what the real adapters produce.
 * The JSON module types are widened (`string` instead of literal unions), hence the casts.
 */
export const fixtures = {
  harnesses: harnessList as unknown as HarnessSummary[],
  recentProjects: projectRecent as unknown as ProjectInfo[],
  config: { omp: configOmp, pi: configPi } as unknown as Record<HarnessId, ConfigView>,
  extensions: { omp: extensionsOmp, pi: extensionsPi } as unknown as Record<HarnessId, ExtensionsView>,
  commands: { omp: commandsOmp, pi: commandsPi } as unknown as Record<HarnessId, CommandSpec[]>,
};

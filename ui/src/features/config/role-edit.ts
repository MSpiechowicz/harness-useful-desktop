import type { ConfigLayer } from '$lib/ipc/bindings/ConfigLayer';
import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
import type { ConfigView } from '$lib/ipc/bindings/ConfigView';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { UnmappedRole } from '$lib/ipc/bindings/UnmappedRole';
import { t } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { configView } from '$lib/stores/views.svelte';

import { selectorRules, type SelectorRules } from './role-selectors';

/** The OMP settings sections, whose Model roles card the unmapped roles belong to (a profile's settings replace the default ones). */
const SETTINGS_SECTIONS: readonly string[] = ['user.settings', 'profile.settings'];

/** The Harness Skills model roles files of Claude Code, OpenCode and Codex, one section per install scope. */
const SKILLS_MODELS_SECTIONS: readonly string[] = ['user.harnessSkillsModels', 'project.harnessSkillsModels'];

const SKILLS_HARNESSES: readonly HarnessId[] = ['claude', 'opencode', 'codex'];

/** What a Model roles card offers to do with its roles. */
export interface RoleEditing {
  /** The harness whose models the picker offers. */
  harness: HarnessId;
  rules: SelectorRules;
  /** The Harness Skills roles the file does not map yet, listed after the rows. */
  unmapped: UnmappedRole[];
  /** Maps an unmapped role to a model, false when it failed. */
  map?: (role: string, selector: string) => Promise<boolean>;
  /** Changes the model of a row, false when it failed. */
  change?: (role: string, selector: string) => Promise<boolean>;
}

/** The Harness Skills roles to offer to map in the section `sectionId` of `view`. */
export function unmappedIn(view: ConfigView, sectionId: string): UnmappedRole[] {
  return view.harness === 'omp' && SETTINGS_SECTIONS.includes(sectionId) ? (view.unmappedRoles ?? []) : [];
}

/** What the Model roles card of `section` can do: map the missing roles of OMP, change the rows of OMP's settings and of a Harness Skills models file. */
export function roleEditingIn(view: ConfigView, section: ConfigSection): RoleEditing | undefined {
  const rules = selectorRules(view.harness);

  if (rules === null) {
    return undefined;
  }

  if (view.harness === 'omp') {
    // The settings file is the one OMP's edits write. A project's settings stay read-only.
    const settings = SETTINGS_SECTIONS.includes(section.id) && section.error === null;

    return { harness: view.harness, rules, unmapped: unmappedIn(view, section.id), map: mapRole, change: settings ? changeOmpRole : undefined };
  }

  const editable = SKILLS_HARNESSES.includes(view.harness) && SKILLS_MODELS_SECTIONS.includes(section.id) && section.error === null;

  if (!editable) {
    return undefined;
  }

  return { harness: view.harness, rules, unmapped: [], change: (role, selector) => changeRole(view.harness, section.source.layer, role, selector) };
}

function fail(title: string, error: unknown): false {
  toasts.push({ title, description: IpcFailure.from(error).message, tone: 'danger' });

  return false;
}

/** Maps a Harness Skills role in OMP's settings. The view re-reads, so the role turns into a normal row. Says why when it fails. */
async function mapRole(role: string, selector: string): Promise<boolean> {
  try {
    await ipc.invoke('omp_map_role', { role, selector });
  } catch (error) {
    return fail(t('components.configView.roles.mapFailed', { role }), error);
  }

  await configView.refresh({ quiet: true });

  return true;
}

/** Changes the model of a role row in OMP's settings. The view re-reads, so the row shows it. Says why when it fails. */
async function changeOmpRole(role: string, selector: string): Promise<boolean> {
  try {
    await ipc.invoke('omp_set_role', { role, selector });
  } catch (error) {
    return fail(t('components.configView.roles.changeFailed', { role }), error);
  }

  await configView.refresh({ quiet: true });

  return true;
}

/** Changes the model of a role row in the Harness Skills models file of an install scope. The view re-reads, so the row shows it. Says why when it fails. */
async function changeRole(harness: HarnessId, scope: ConfigLayer, role: string, selector: string): Promise<boolean> {
  try {
    await ipc.invoke('skills_set_role', { harness, scope, project: project.active?.path ?? null, role, selector });
  } catch (error) {
    return fail(t('components.configView.roles.changeFailed', { role }), error);
  }

  await configView.refresh({ quiet: true });

  return true;
}

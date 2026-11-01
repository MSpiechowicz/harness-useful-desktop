import type { ConfigView } from '../bindings/ConfigView';
import type { ConfigLayer } from '../bindings/ConfigLayer';
import type { HarnessId } from '../bindings/HarnessId';
import { IpcFailure } from '../transport';

/** The model roles the preview's Harness Skills files hold, changed until the preview is reloaded. */
export class MockSkillsRoles {
  private readonly changed = new Map<string, string>();

  private static key(harness: HarnessId, layer: ConfigLayer, role: string): string {
    return `${harness}/${layer}/${role}`;
  }

  change(harness: HarnessId, scope: ConfigLayer, role: string, selector: string, view: ConfigView): null {
    if (!/^\S+$/.test(selector)) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'The model must look like model or model:effort.' });
    }

    const exists = view.sections.some(
      (section) => section.source.layer === scope && section.id.endsWith('harnessSkillsModels') && this.rolesOf(section.entries).some((field) => field.key === role),
    );

    if (!exists) {
      throw IpcFailure.from({ code: 'notFound', message: `The ${role} row of the model roles was not found.` });
    }

    this.changed.set(MockSkillsRoles.key(harness, scope, role), selector);

    return null;
  }

  /** The view with the rows changed so far. */
  decorate(view: ConfigView): ConfigView {
    if (this.changed.size === 0) {
      return view;
    }

    const sections = view.sections.map((section) => {
      if (!section.id.endsWith('harnessSkillsModels')) {
        return section;
      }

      return {
        ...section,
        entries: section.entries.map((field) => {
          if (field.key !== 'modelRoles' || field.value.kind !== 'map') {
            return field;
          }

          const rows = field.value.value.map((row) => {
            const selector = this.changed.get(MockSkillsRoles.key(view.harness, section.source.layer, row.key));

            return selector === undefined ? row : { ...row, value: { kind: 'text' as const, value: selector } };
          });

          return { ...field, value: { ...field.value, value: rows } };
        }),
      };
    });

    return { ...view, sections };
  }

  private rolesOf(entries: ConfigView['sections'][number]['entries']): { key: string }[] {
    const roles = entries.find((field) => field.key === 'modelRoles');

    return roles?.value.kind === 'map' ? roles.value.value : [];
  }
}

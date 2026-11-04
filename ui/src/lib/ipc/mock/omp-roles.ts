import type { ConfigView } from '../bindings/ConfigView';
import type { UnmappedRole } from '../bindings/UnmappedRole';
import { IpcFailure } from '../transport';

/** The Harness Skills roles the preview's OMP settings do not map yet, until the preview maps them. */
const SKILLS_ROLES: readonly UnmappedRole[] = [
  { role: 'review', inheritsSessionModel: false },
  { role: 'assess', inheritsSessionModel: true },
];

const SELECTOR = /^[A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._:@+/-]*$/;

/** The OMP model roles of the preview: Harness Skills roles that stay unmapped until `omp_map_role` maps them. */
export class MockOmpRoles {
  private readonly mapped = new Map<string, string>();
  /** Models set on the roles the preview's settings map already. */
  private readonly changed = new Map<string, string>();

  map(role: string, selector: string): null {
    if (!SKILLS_ROLES.some((known) => known.role === role)) {
      throw IpcFailure.from({ code: 'invalidInput', message: `${role} is not a Harness Skills role.` });
    }

    if (!SELECTOR.test(selector)) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'The model must look like provider/model or provider/model:thinking.' });
    }

    this.mapped.set(role, selector);

    return null;
  }

  change(role: string, selector: string): null {
    if (!SELECTOR.test(selector)) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'The model must look like provider/model or provider/model:thinking.' });
    }

    if (this.mapped.has(role)) {
      this.mapped.set(role, selector);
    } else {
      this.changed.set(role, selector);
    }

    return null;
  }

  /** The OMP view with the roles mapped so far in `modelRoles` and the others offered as unmapped. */
  decorate(view: ConfigView): ConfigView {
    const sections = view.sections.map((section) => {
      if (section.id !== 'user.settings') {
        return section;
      }

      return {
        ...section,
        entries: section.entries.map((field) => {
          if (field.key !== 'modelRoles' || field.value.kind !== 'map') {
            return field;
          }

          const added = [...this.mapped].map(([key, value]) => ({ key, value: { kind: 'text' as const, value } }));
          const kept = field.value.value.map((entry) => {
            const value = this.changed.get(entry.key);

            return value === undefined ? entry : { ...entry, value: { kind: 'text' as const, value } };
          });

          return { ...field, value: { ...field.value, value: [...kept, ...added] } };
        }),
      };
    });

    return { ...view, sections, unmappedRoles: SKILLS_ROLES.filter((known) => !this.mapped.has(known.role)) };
  }
}

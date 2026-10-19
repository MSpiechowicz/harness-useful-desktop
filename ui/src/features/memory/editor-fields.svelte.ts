import { t } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';

/** Why a field is not acceptable. A `required` one is only said once the user has been at the field. */
export interface Problem<F extends string> {
  field: F;
  message: MessageKey;
  params?: Record<string, number>;
  required: boolean;
}

/**
 * Which problems of a form to say: those of fields the user has been at, and those that are not merely "still empty".
 * Call it while an editor initialises. `read` gives all the problems of the form as it is now. Every time the editor opens (`open`) no field has been visited.
 */
export function fieldProblems<F extends string>(read: () => Problem<F>[], open: () => boolean) {
  let touched = $state<Partial<Record<F, boolean>>>({});
  const shown = $derived(read().filter((problem) => !problem.required || touched[problem.field]));

  $effect.pre(() => {
    if (open()) {
      touched = {};
    }
  });

  return {
    /** Something in the form keeps it from being saved. */
    get blocked(): boolean {
      return read().length > 0;
    },

    /** The message of the first problem said for a field, or null. */
    messageOf(field: F): string | null {
      const problem = shown.find((candidate) => candidate.field === field);

      return problem ? t(problem.message, problem.params) : null;
    },

    touch(field: F): void {
      touched = { ...touched, [field]: true };
    },

    /** Every field counts as visited, e.g. once a save was asked for. */
    touchAll(fields: readonly F[]): void {
      touched = Object.fromEntries(fields.map((field) => [field, true])) as Partial<Record<F, boolean>>;
    },
  };
}

/** Runs `begin` each time the editor opens, so it starts from a fresh form. Call it while an editor initialises. */
export function onOpen(open: () => boolean, begin: () => void): void {
  $effect.pre(() => {
    if (open()) {
      begin();
    }
  });
}

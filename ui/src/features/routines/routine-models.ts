import type { ModelOption } from '$lib/ipc/bindings/ModelOption';

export interface ModelRef {
  provider: string;
  id: string;
}

export function sameModel(a: ModelRef | null, b: ModelRef | null): boolean {
  return a !== null && b !== null && a.provider === b.provider && a.id === b.id;
}

/**
 * The models a harness lists, with the one a routine already names added when the harness does not list it (it may not be
 * signed in any more): the routine keeps showing it, as its id, so nothing is lost by opening the editor.
 */
export function withSaved(models: ModelOption[], saved: ModelRef | null): ModelOption[] {
  if (saved === null || models.some((model) => sameModel(model, saved))) {
    return models;
  }

  return [{ provider: saved.provider, id: saved.id, name: null, reasoning: true, contextWindow: null, maxTokens: null, input: [] }, ...models];
}

/** The levels a harness lists for a model, with the one a routine already names added when the harness does not list it. */
export function levelsWithSaved(levels: string[], saved: string): string[] {
  return saved === '' || levels.includes(saved) ? levels : [saved, ...levels];
}

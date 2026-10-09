import type { ModelOption } from '$lib/ipc/bindings/ModelOption';

export interface ModelGroup {
  provider: string;
  models: ModelOption[];
}

/** Models grouped by provider, in the order the providers first appear. */
export function groupByProvider(models: ModelOption[]): ModelGroup[] {
  const groups = new Map<string, ModelOption[]>();

  for (const model of models) {
    groups.set(model.provider, [...(groups.get(model.provider) ?? []), model]);
  }

  return [...groups].map(([provider, members]) => ({ provider, models: members }));
}

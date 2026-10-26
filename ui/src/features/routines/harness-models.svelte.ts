import { withHelper } from '$lib/chat/helper-session';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { ModelOption } from '$lib/ipc/bindings/ModelOption';
import type { Transport } from '$lib/ipc/transport';

/** The harness and the folder a throwaway agent is asked in: the models a harness offers can depend on the folder (its project settings). */
export interface ModelScope {
  harness: HarnessId;
  project: string;
}

const scopeKey = (scope: ModelScope): string => `${scope.harness}\n${scope.project}`;
const levelsKey = (scope: ModelScope, model: { provider: string; id: string }): string => `${scopeKey(scope)}\n${model.provider}\n${model.id}`;

/**
 * The models of a harness and the thinking levels of each, for an editor that has no chat to ask. Each is asked of a
 * short-lived helper agent (the same one that names chats), once per harness and folder while the editor is open: a
 * second look at what was answered costs nothing. An ask that failed leaves nothing behind but `failed`, so the next
 * look asks again.
 */
export class HarnessModels {
  private lists = $state.raw<Record<string, ModelOption[]>>({});
  private levels = $state.raw<Record<string, string[]>>({});
  private broken = $state.raw<Record<string, boolean>>({});
  private asking = new Map<string, Promise<void>>();

  constructor(private readonly transport: Transport) {}

  /** The models of the scope; null until they are known. */
  modelsOf(scope: ModelScope): ModelOption[] | null {
    return this.lists[scopeKey(scope)] ?? null;
  }

  /** The thinking levels of the model in the scope; null until they are known. */
  levelsOf(scope: ModelScope, model: { provider: string; id: string }): string[] | null {
    return this.levels[levelsKey(scope, model)] ?? null;
  }

  /** The last ask for the models, or for the levels of `model`, failed: the harness could not be asked here. */
  failed(scope: ModelScope, model?: { provider: string; id: string }): boolean {
    return this.broken[model ? levelsKey(scope, model) : scopeKey(scope)] === true;
  }

  loadModels(scope: ModelScope): Promise<void> {
    const key = scopeKey(scope);

    return this.ask(
      key,
      this.lists[key] !== undefined,
      () =>
        withHelper(this.transport, scope, async (helper) => {
          const reply = await helper.send({ type: 'getAvailableModels' });

          return reply.type === 'models' ? reply.models : null;
        }),
      (models) => (this.lists = { ...this.lists, [key]: models }),
    );
  }

  /** The levels belong to a model: the helper switches to it first. */
  loadLevels(scope: ModelScope, model: { provider: string; id: string }): Promise<void> {
    const key = levelsKey(scope, model);

    return this.ask(
      key,
      this.levels[key] !== undefined,
      () =>
        withHelper(this.transport, scope, async (helper) => {
          await helper.send({ type: 'setModel', provider: model.provider, modelId: model.id });
          const reply = await helper.send({ type: 'getAvailableThinkingLevels' });

          return reply.type === 'thinkingLevels' ? reply.levels : null;
        }),
      (levels) => (this.levels = { ...this.levels, [key]: levels }),
    );
  }

  /** Asks unless it is known or being asked already. */
  private ask<T>(key: string, known: boolean, read: () => Promise<T | null>, keep: (answer: T) => void): Promise<void> {
    const running = this.asking.get(key);

    if (known || running) {
      return running ?? Promise.resolve();
    }

    this.broken = { ...this.broken, [key]: false };

    const asked = read()
      .then((answer) => {
        if (answer === null) {
          this.broken = { ...this.broken, [key]: true };
        } else {
          keep(answer);
        }
      })
      .finally(() => this.asking.delete(key));

    this.asking.set(key, asked);

    return asked;
  }
}

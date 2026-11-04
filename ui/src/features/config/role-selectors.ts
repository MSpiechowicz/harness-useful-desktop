import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { ModelOption } from '$lib/ipc/bindings/ModelOption';
import type { MessageKey } from '$lib/i18n/i18n.svelte';

/** What a model role row of a harness accepts and how the picker words it. */
export interface SelectorRules {
  isValid: (text: string) => boolean;
  placeholder: MessageKey;
  invalid: MessageKey;
  /** Values every file takes besides a model, offered as suggestions. */
  aliases: readonly string[];
  /** How a model the harness offers is written in a role row. */
  selectorOf: (model: { provider: string; id: string }) => string;
  /** Every thinking level a row may end with (`model:level`); the ones a model takes are asked of the harness. */
  levels: readonly string[];
  /** The models that take no level at all, such as `inherit`. */
  levelless: readonly string[];
  /** Levels offered for a model the harness does not list (a Claude Code alias such as `opus`); empty when there are none to offer. */
  unlistedLevels: readonly string[];
  /** Whether the model part of a row names the provider (`provider/model`), so the model to ask about is read from the row itself. */
  namesProvider: boolean;
}

/** A role row's model and thinking level. */
export interface SelectorParts {
  model: string;
  level: string | null;
}

/** `text` split into its model and a trailing `:level` the harness knows. */
export function splitSelector(rules: SelectorRules, text: string): SelectorParts {
  const colon = text.lastIndexOf(':');
  const suffix = colon > 0 ? text.slice(colon + 1) : '';

  return rules.levels.includes(suffix) ? { model: text.slice(0, colon), level: suffix } : { model: text, level: null };
}

/** `model` with `level` appended, or bare when there is none or the model takes none. */
export function joinSelector(rules: SelectorRules, model: string, level: string | null): string {
  return level === null || rules.levelless.includes(model) ? model : `${model}:${level}`;
}

/** The model to ask the harness about for the levels of `model` (a row's model part), null when it is not one the harness lists. */
export function modelToAsk(rules: SelectorRules, model: string, listed: readonly ModelOption[] | null): { provider: string; id: string } | null {
  if (rules.levelless.includes(model)) {
    return null;
  }

  if (rules.namesProvider) {
    const slash = model.indexOf('/');

    return slash > 0 ? { provider: model.slice(0, slash), id: model.slice(slash + 1) } : null;
  }

  // A bare-ID row may still name the provider (Claude Code takes `anthropic/claude-…`).
  const option = listed?.find((candidate) => [rules.selectorOf(candidate), `${candidate.provider}/${rules.selectorOf(candidate)}`].includes(model));

  return option ? { provider: option.provider, id: option.id } : null;
}

const withProvider = (model: { provider: string; id: string }): string => `${model.provider}/${model.id}`;
const bareId = (model: { provider: string; id: string }): string => model.id;

const CLAUDE_MODEL = /^(inherit|opus|sonnet|haiku|fable|claude-[a-z0-9][a-z0-9.-]{0,80})$/;
const CLAUDE_EFFORTS = ['low', 'medium', 'high', 'xhigh', 'max'];
const OPENCODE_VARIANTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];
const CODEX_EFFORTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'];
/** The thinking levels OMP reads after a role's model. */
const OMP_LEVELS = ['off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];
const PROVIDER = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const MODEL = /^[A-Za-z0-9](?:[A-Za-z0-9._:/@+-]{0,198}[A-Za-z0-9._@+-])?$/;
const OMP = /^[A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._:@+/-]*$/;

/** `selector` without its last `:suffix` when that is one of `known`. */
function withoutSuffix(selector: string, known: readonly string[]): string {
  const colon = selector.lastIndexOf(':');

  return colon >= 0 && known.includes(selector.slice(colon + 1)) ? selector.slice(0, colon) : selector;
}

function isClaude(text: string): boolean {
  const [model, effort, extra] = text.replace(/^anthropic\//, '').split(':');

  return extra === undefined && CLAUDE_MODEL.test(model) && (effort === undefined || CLAUDE_EFFORTS.includes(effort));
}

function isOpenCode(text: string): boolean {
  if (text === 'inherit') {
    return true;
  }

  const model = withoutSuffix(text, OPENCODE_VARIANTS);
  const slash = model.indexOf('/');

  return slash > 0 && PROVIDER.test(model.slice(0, slash)) && MODEL.test(model.slice(slash + 1));
}

function isCodex(text: string): boolean {
  const model = withoutSuffix(text, CODEX_EFFORTS);

  return model === 'inherit' || MODEL.test(model);
}

const RULES: Partial<Record<HarnessId, SelectorRules>> = {
  omp: {
    isValid: (text) => OMP.test(text) && !text.endsWith(':'),
    placeholder: 'components.configView.roles.placeholderOmp',
    invalid: 'components.configView.roles.invalidOmp',
    aliases: [],
    selectorOf: withProvider,
    levels: OMP_LEVELS,
    levelless: [],
    unlistedLevels: [],
    namesProvider: true,
  },
  claude: {
    isValid: isClaude,
    placeholder: 'components.configView.roles.placeholderClaude',
    invalid: 'components.configView.roles.invalidClaude',
    aliases: ['opus', 'sonnet', 'haiku', 'fable', 'inherit'],
    selectorOf: bareId,
    levels: CLAUDE_EFFORTS,
    levelless: ['inherit'],
    unlistedLevels: CLAUDE_EFFORTS,
    namesProvider: false,
  },
  opencode: {
    isValid: isOpenCode,
    placeholder: 'components.configView.roles.placeholderOpenCode',
    invalid: 'components.configView.roles.invalidOpenCode',
    aliases: ['inherit'],
    selectorOf: withProvider,
    levels: OPENCODE_VARIANTS,
    levelless: ['inherit'],
    unlistedLevels: [],
    namesProvider: true,
  },
  codex: {
    isValid: isCodex,
    placeholder: 'components.configView.roles.placeholderCodex',
    invalid: 'components.configView.roles.invalidCodex',
    aliases: ['inherit'],
    selectorOf: bareId,
    levels: CODEX_EFFORTS,
    levelless: ['inherit'],
    unlistedLevels: CODEX_EFFORTS,
    namesProvider: false,
  },
};

/** The rules of the model role rows of `harness`, or null for a harness without them. */
export function selectorRules(harness: HarnessId): SelectorRules | null {
  return RULES[harness] ?? null;
}

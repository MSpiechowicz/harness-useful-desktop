import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { MessageKey } from '$lib/i18n/i18n.svelte';

/** What a model role row of a harness accepts and how the picker words it. */
export interface SelectorRules {
  isValid: (text: string) => boolean;
  placeholder: MessageKey;
  invalid: MessageKey;
  /** Values every file takes besides a model, offered as suggestions. */
  aliases: readonly string[];
}

const CLAUDE_MODEL = /^(inherit|opus|sonnet|haiku|fable|claude-[a-z0-9][a-z0-9.-]{0,80})$/;
const CLAUDE_EFFORTS = ['low', 'medium', 'high', 'xhigh', 'max'];
const OPENCODE_VARIANTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];
const CODEX_EFFORTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'];
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
  },
  claude: {
    isValid: isClaude,
    placeholder: 'components.configView.roles.placeholderClaude',
    invalid: 'components.configView.roles.invalidClaude',
    aliases: ['opus', 'sonnet', 'haiku', 'fable', 'inherit'],
  },
  opencode: {
    isValid: isOpenCode,
    placeholder: 'components.configView.roles.placeholderOpenCode',
    invalid: 'components.configView.roles.invalidOpenCode',
    aliases: ['inherit'],
  },
  codex: {
    isValid: isCodex,
    placeholder: 'components.configView.roles.placeholderCodex',
    invalid: 'components.configView.roles.invalidCodex',
    aliases: ['inherit'],
  },
};

/** The rules of the model role rows of `harness`, or null for a harness without them. */
export function selectorRules(harness: HarnessId): SelectorRules | null {
  return RULES[harness] ?? null;
}

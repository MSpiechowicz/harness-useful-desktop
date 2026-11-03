import type { Hook } from '../bindings/Hook';
import { STEPS_PART } from '../../chat/hook-context';
import { CATALOG, SAMPLE_MEMORY_RECALL } from './hooks-fixtures';

/** A name as the app writes it into a hook element: `& < > "` are escaped. */
export function escapeName(name: string): string {
  return name.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}

/** A text as the app writes it: a `<` that begins a hook marker is escaped. */
export function escapeText(text: string): string {
  return text.replace(/<(?=\/?(?:hook|harness-hook))/gi, '&lt;');
}

const stepOf = (hook: Hook): number | null => CATALOG.find((entry) => entry.seed === hook.seed)?.step ?? null;
const textOf = (hook: Hook): string => (hook.action.type === 'command' ? '' : hook.action.text);

/**
 * The context block of a first message for the active context `hooks`: an intro and the Harness family steps numbered from 1
 * in catalog order (the Harness Memory step carries a sample of the notes the app finds), then the other `sessionStart` hooks.
 * Empty when there is nothing to say.
 */
export function composeContext(hooks: Hook[]): string {
  const steps = hooks.filter((hook) => stepOf(hook) !== null).sort((a, b) => (stepOf(a) ?? 0) - (stepOf(b) ?? 0));
  const others = hooks.filter((hook) => stepOf(hook) === null && hook.event === 'sessionStart');

  const elements = [
    ...(steps.length === 0 ? [] : [{ name: STEPS_PART, text: `Work through these ${steps.length} steps in order, then do the request below.` }]),
    ...steps.map((hook, index) => ({ name: hook.name, text: `${index + 1}. ${textOf(hook)}${hook.seed === 'memory-first' ? `\n${SAMPLE_MEMORY_RECALL}` : ''}` })),
    ...others.map((hook) => ({ name: hook.name, text: textOf(hook) })),
  ];

  if (elements.length === 0) {
    return '';
  }

  const lines = elements.map(({ name, text }) => `<hook name="${escapeName(name)}">${escapeText(text)}</hook>`);

  return `<harness-hook-context>\n${lines.join('\n')}\n</harness-hook-context>\n\n`;
}

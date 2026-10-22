import type { Capabilities } from '$lib/ipc/bindings/Capabilities';
import type { MessageKey } from '$lib/i18n/locales/en';

/** The capabilities worth naming on a harness's introduction, in the order they are listed. Chat is not one: a harness without it is greyed out on its card instead. */
const TAGS: { capability: keyof Capabilities; label: MessageKey }[] = [
  { capability: 'plugins', label: 'onboarding.meet.tag.plugins' },
  { capability: 'packages', label: 'onboarding.meet.tag.packages' },
  { capability: 'mcp', label: 'onboarding.meet.tag.mcp' },
  { capability: 'skills', label: 'onboarding.meet.tag.skills' },
  { capability: 'profiles', label: 'onboarding.meet.tag.profiles' },
];

/** No more than fit on a card's row without crowding it. */
const MAX_TAGS = 3;

/** The labels of what a harness can do, read from its capabilities, never written by hand per harness. */
export function featureTags(capabilities: Capabilities): MessageKey[] {
  return TAGS.filter((tag) => capabilities[tag.capability])
    .slice(0, MAX_TAGS)
    .map((tag) => tag.label);
}

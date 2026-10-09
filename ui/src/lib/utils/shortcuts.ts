/**
 * Keyboard shortcut helpers. A combo is a lowercase string such as `mod+k`,
 * `mod+,` or `alt+arrowleft`; `mod` is Cmd on macOS and Ctrl elsewhere.
 */

const MODIFIERS = ['mod', 'alt', 'shift'] as const;

export function isMacPlatform(): boolean {
  return /Mac|iPhone|iPad/i.test(navigator.userAgent);
}

interface ParsedCombo {
  key: string;
  mod: boolean;
  alt: boolean;
  shift: boolean;
}

function parseCombo(combo: string): ParsedCombo {
  const parts = combo.toLowerCase().split('+');
  const key = parts[parts.length - 1];
  const modifiers = new Set(parts.slice(0, -1));

  return { key, mod: modifiers.has('mod'), alt: modifiers.has('alt'), shift: modifiers.has('shift') };
}

export function matchesShortcut(event: KeyboardEvent, combo: string, mac = isMacPlatform()): boolean {
  const wanted = parseCombo(combo);
  const modPressed = mac ? event.metaKey : event.ctrlKey;
  const strayModifier = mac ? event.ctrlKey : event.metaKey;

  if (strayModifier) {
    return false;
  }
  if (modPressed !== wanted.mod || event.altKey !== wanted.alt || event.shiftKey !== wanted.shift) {
    return false;
  }

  return event.key.toLowerCase() === wanted.key;
}

const SYMBOLS_MAC: Record<string, string> = { mod: '⌘', alt: '⌥', shift: '⇧' };
const SYMBOLS_OTHER: Record<string, string> = { mod: 'Ctrl', alt: 'Alt', shift: 'Shift' };
const KEY_LABELS: Record<string, string> = {
  arrowleft: '←',
  arrowright: '→',
  arrowup: '↑',
  arrowdown: '↓',
  enter: '↵',
  escape: 'Esc',
};

/** Display tokens for a combo, one per key cap. */
export function formatShortcut(combo: string, mac = isMacPlatform()): string[] {
  const symbols = mac ? SYMBOLS_MAC : SYMBOLS_OTHER;
  const parsed = parseCombo(combo);
  const caps = MODIFIERS.filter((name) => parsed[name]).map((name) => symbols[name]);
  const keyLabel = KEY_LABELS[parsed.key] ?? parsed.key.toUpperCase();

  return [...caps, keyLabel];
}

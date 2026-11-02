/** Control characters, the line and paragraph separators, the format characters (bidi controls, zero-width characters) and the other invisible ones: they draw nothing or reorder the text around them. */
const HIDDEN = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}͏ᅟᅠ឴឵᠋-᠏⠀ㅤ︀-️ﾠ\u{E0100}-\u{E01EF}]/gu;

/** `text` of another program as it can be shown: without hidden characters, trimmed, at most `max` characters (not UTF-16 units) long. The counterpart of the business `display_text`. */
export function displayText(text: string, max: number): string {
  return [...text.trim().replace(HIDDEN, '')].slice(0, max).join('').trim();
}

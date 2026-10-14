import type { Notice } from '$lib/ipc/bindings/Notice';

export interface ProfileNotice {
  /** Profile names the backend listed. */
  names: string[];
  /** The sentence that says how to use one, as the backend wrote it; null when it sent none. */
  hint: string | null;
  /** Every notice that is not the profile list, in order. */
  rest: Notice[];
}

// The backend reports the profiles as one info notice: "Profiles: a, b. Start OMP with --profile <name> to use one."
const PROFILES = /^Profiles:\s+(.+?)\.(?:\s+(\S.*))?$/s;

/** Pulls the profile list out of a view's notices so it can be shown as a card; anything else stays a notice. */
export function splitProfileNotice(notices: Notice[]): ProfileNotice {
  for (const [index, notice] of notices.entries()) {
    const found = notice.level === 'info' ? PROFILES.exec(notice.message.trim()) : null;
    const names = found?.[1].split(',').map((name) => name.trim()).filter(Boolean) ?? [];

    if (found && names.length > 0) {
      return { names, hint: found[2]?.trim() || null, rest: notices.filter((_, other) => other !== index) };
    }
  }

  return { names: [], hint: null, rest: notices };
}

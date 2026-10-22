import { t } from '$lib/i18n/i18n.svelte';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { NativeApprovalMode } from '$lib/ipc/bindings/NativeApprovalMode';

export type ModeTone = 'warning' | 'success' | 'info';

export interface ModeInfo {
  label: string;
  tone: ModeTone;
  /** What the mode means for the user, also the badge tooltip. */
  explanation: string;
}

/** The words are getters so they follow the language on screen. */
export const MODES: Record<ApprovalMode, ModeInfo> = {
  yolo: {
    get label() {
      return t('chat.mode.yolo.label');
    },
    tone: 'warning',
    get explanation() {
      return t('chat.mode.yolo.explanation');
    },
  },
  write: {
    get label() {
      return t('chat.mode.write.label');
    },
    tone: 'info',
    get explanation() {
      return t('chat.mode.write.explanation');
    },
  },
  alwaysAsk: {
    get label() {
      return t('chat.mode.alwaysAsk.label');
    },
    tone: 'success',
    get explanation() {
      return t('chat.mode.alwaysAsk.explanation');
    },
  },
};

export const MODE_ORDER: ApprovalMode[] = ['alwaysAsk', 'write', 'yolo'];

/** The mode in force: the per-session override, else the harness configuration. */
export function activeMode(override: ApprovalMode | null, configured: ApprovalMode | null): ApprovalMode | null {
  return override ?? configured;
}

/** Claude Code's own permission modes by the name it uses in its settings. */
const NATIVE_LABELS: Record<string, () => string> = {
  default: () => t('chat.nativeMode.default'),
  acceptEdits: () => t('chat.nativeMode.acceptEdits'),
  plan: () => t('chat.nativeMode.plan'),
  auto: () => t('chat.nativeMode.auto'),
  dontAsk: () => t('chat.nativeMode.dontAsk'),
  bypassPermissions: () => t('chat.nativeMode.bypassPermissions'),
};

/** The words for a harness's own mode. A name this version does not know shows as the harness wrote it. */
export function nativeModeLabel(native: NativeApprovalMode): string {
  return NATIVE_LABELS[native.name]?.() ?? native.name;
}

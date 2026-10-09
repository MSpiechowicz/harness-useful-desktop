import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';

export type ModeTone = 'warning' | 'success' | 'info';

export interface ModeInfo {
  label: string;
  tone: ModeTone;
  /** What the mode means for the user, also the badge tooltip. */
  explanation: string;
}

export const MODES: Record<ApprovalMode, ModeInfo> = {
  yolo: { label: 'yolo', tone: 'warning', explanation: 'tools run without asking' },
  write: { label: 'ask for writes', tone: 'info', explanation: 'asks before tools that change files or run commands' },
  alwaysAsk: { label: 'ask every time', tone: 'success', explanation: 'asks before every tool call' },
};

export const MODE_ORDER: ApprovalMode[] = ['alwaysAsk', 'write', 'yolo'];

/** The mode in force: the per-session override, else the harness configuration. */
export function activeMode(override: ApprovalMode | null, configured: ApprovalMode | null): ApprovalMode | null {
  return override ?? configured;
}

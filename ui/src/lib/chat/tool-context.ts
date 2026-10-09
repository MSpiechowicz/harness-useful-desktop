import { getContext, setContext } from 'svelte';

/** Where a transcript's tool cards belong, so "View changes" opens the right project's diff. */
export interface ToolContext {
  /** Absolute project path of the chat or saved session. */
  project: string;
  /** How the Changes view names the source of the file filter: "this chat", "this session". */
  label: string;
}

const KEY = Symbol('tool-context');

export function provideToolContext(context: ToolContext): void {
  setContext(KEY, context);
}

export function toolContext(): ToolContext | undefined {
  return getContext<ToolContext | undefined>(KEY);
}

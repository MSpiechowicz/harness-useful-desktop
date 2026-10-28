/** Within this of the end counts as at the end: a person who scrolled to the bottom is there to the pixel or two. */
const NEAR_END_PX = 48;

export function nearEnd(scroller: HTMLElement): boolean {
  return scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < NEAR_END_PX;
}

/** The scrolling part of a side panel that holds `node`. */
export function panelScroller(node: HTMLElement | undefined): HTMLElement | null {
  return node?.closest<HTMLElement>('[data-panel-body]') ?? null;
}

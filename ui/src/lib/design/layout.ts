/**
 * Layout breakpoints and minimums that script needs as numbers (CSS reads the lengths from tokens.css).
 * Each is measured on the width of the view itself, not of the window, so the rail and the page chrome never skew it.
 */

/** Below this width the list of a workspace view moves into a drawer so the centre keeps its room. */
export const DRAWER_BELOW_PX = 1100;

/** From this width a workspace view has room for the details as a third column. Below it they slide over the centre. */
export const DETAILS_INLINE_FROM_PX = 1560;

/** From this width Hooks and Memory have room for the table and an always-present detail column. Below it the detail slides over. */
export const WIDE_FROM_PX = 1360;

/** The narrowest a workspace list column can be dragged. */
export const LIST_MIN_PX = 240;

/** The room the centre of a workspace view keeps when the list is dragged wide. */
export const CENTRE_MIN_PX = 480;

import type { Component } from 'svelte';
import type { LucideProps } from '@lucide/svelte';

/** Any Lucide icon component, e.g. `import Search from '@lucide/svelte/icons/search'`. */
export type IconComponent = Component<LucideProps, {}, ''>;

/**
 * The icon size scale, in px: `sm` beside text and in dense controls, `md` in buttons and rows,
 * `lg` for navigation. Illustrations (empty states, hero tiles) are the only larger icons.
 */
export const ICON = { sm: 16, md: 18, lg: 20 } as const;

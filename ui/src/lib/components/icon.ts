import type { Component } from 'svelte';
import type { LucideProps } from '@lucide/svelte';

/** Any Lucide icon component, e.g. `import Search from '@lucide/svelte/icons/search'`. */
export type IconComponent = Component<LucideProps, {}, ''>;

import CircleAlert from '@lucide/svelte/icons/circle-alert';
import Info from '@lucide/svelte/icons/info';
import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

import type { NoticeLevel } from '$lib/ipc/bindings/NoticeLevel';

import type { IconComponent } from './icon';

/** Icon, wash and text color of each notice level; shared by every place that shows a notice. */
export const NOTICE_LOOKS: Record<NoticeLevel, { icon: IconComponent; box: string; color: string }> = {
  info: { icon: Info, box: 'bg-info-wash', color: 'text-info' },
  warning: { icon: TriangleAlert, box: 'bg-warning-wash', color: 'text-warning' },
  error: { icon: CircleAlert, box: 'bg-danger-wash', color: 'text-danger' },
};

<script lang="ts">
  import { ICON } from './icon';

  interface Props {
    /** Which side of the frame the panel is on. */
    side: 'left' | 'right';
    /** The panel is open: its side of the frame is filled. */
    filled: boolean;
    size?: number;
  }

  let { side, filled, size = ICON.md }: Props = $props();

  const LEFT = { fill: 3, divider: 9 } as const;
  const RIGHT = { fill: 15, divider: 15 } as const;

  const at = $derived(side === 'left' ? LEFT : RIGHT);
</script>

<!-- Lucide's panel icon with the side area filled while the panel is open: the frame says where the panel is, the fill whether it shows. -->
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <rect x="3" y="3" width="18" height="18" rx="2" />
  <rect x={at.fill} y="3" width="6" height="18" rx="1" fill="currentColor" stroke="none" class="transition-opacity duration-[var(--dur-fast)]" opacity={filled ? 0.9 : 0} />
  <path d="M{at.divider} 3v18" />
</svg>

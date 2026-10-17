<script lang="ts">
  import { ICON, type IconComponent } from '$lib/components/icon';
  import { rise } from '$lib/design/motion';

  interface Props {
    icon: IconComponent;
    /** What the empty column of a wide view says until a row is chosen. */
    message: string;
  }

  let { icon: Icon, message }: Props = $props();
</script>

<div in:rise={{ y: 6, duration: 240 }} class="grid min-h-0 flex-1 place-items-center p-8">
  <div class="flex max-w-60 flex-col items-center gap-3 text-center">
    <Icon size={ICON.lg} strokeWidth={1.6} class="text-fg-3" aria-hidden="true" />
    <p class="text-sm text-fg-3">{message}</p>
  </div>
</div>

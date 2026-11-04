<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import Info from '@lucide/svelte/icons/info';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import X from '@lucide/svelte/icons/x';

  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ToastItem, ToastTone } from '$lib/stores/toasts.svelte';
  import { cx } from '$lib/utils/cx';

  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';
  import type { IconComponent } from './icon';

  interface Props {
    toast: ToastItem;
    ondismiss: (id: number) => void;
  }

  let { toast, ondismiss }: Props = $props();

  const ICONS: Record<ToastTone, { icon: IconComponent; color: string }> = {
    neutral: { icon: Info, color: 'text-fg-2' },
    info: { icon: Info, color: 'text-info' },
    success: { icon: CircleCheck, color: 'text-success' },
    warning: { icon: TriangleAlert, color: 'text-warning' },
    danger: { icon: CircleAlert, color: 'text-danger' },
  };

  const tone = $derived(ICONS[toast.tone]);
  const Icon = $derived(tone.icon);

  let paused = $state(false);

  // Auto-dismiss; hovering or focusing the toast pauses the countdown, and an update of the toast starts it over.
  $effect(() => {
    void toast.revision;

    if (toast.duration === 0 || paused) {
      return;
    }

    const timer = setTimeout(() => ondismiss(toast.id), toast.duration);

    return () => clearTimeout(timer);
  });
</script>

<div
  role={toast.tone === 'danger' ? 'alert' : 'status'}
  transition:rise={{ y: 12, duration: 260 }}
  onpointerenter={() => (paused = true)}
  onpointerleave={() => (paused = false)}
  onfocusin={() => (paused = true)}
  onfocusout={() => (paused = false)}
  class="pointer-events-auto flex w-80 items-start gap-3 rounded-lg border border-line-strong bg-glass p-3.5 shadow-pop backdrop-blur-xl"
>
  <!-- The icon and the close button sit on the optical middle of the title's letters, one pixel under the middle of its
       20px line box; the button's negative margins keep the 36px control from making the toast taller. -->
  <Icon class={cx(tone.color, 'mt-[2px] shrink-0')} size={18} aria-hidden="true" />

  <div class="flex min-w-0 flex-1 flex-col gap-0.5">
    <!-- Every toast is a small title with a line that says more under it. -->
    <p class="text-sm leading-[20px] font-medium text-fg">{toast.title}</p>
    <p class="text-sm break-words text-fg-2">{toast.description.trim() || t('components.toast.noDetails')}</p>
    {#if toast.action}
      {@const action = toast.action}
      <Button class="mt-1.5 self-start" onclick={() => { action.run(); ondismiss(toast.id); }}>{action.label}</Button>
    {/if}
  </div>

  <IconButton icon={X} label={t('components.toast.dismiss')} class="-mt-[7px] -mb-[9px] -mr-1.5 shrink-0" onclick={() => ondismiss(toast.id)} />
</div>

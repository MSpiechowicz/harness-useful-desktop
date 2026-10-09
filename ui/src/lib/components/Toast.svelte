<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import Info from '@lucide/svelte/icons/info';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import X from '@lucide/svelte/icons/x';

  import { rise } from '$lib/design/motion';
  import type { ToastItem, ToastTone } from '$lib/stores/toasts.svelte';

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

  // Auto-dismiss; hovering or focusing the toast pauses the countdown.
  $effect(() => {
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
  <Icon class={tone.color} size={18} aria-hidden="true" />

  <div class="flex min-w-0 flex-1 flex-col gap-0.5">
    <p class="text-base font-medium text-fg">{toast.title}</p>
    {#if toast.description}
      <p class="text-sm break-words text-fg-2">{toast.description}</p>
    {/if}
    {#if toast.action}
      {@const action = toast.action}
      <Button size="sm" class="mt-1.5 self-start" onclick={() => { action.run(); ondismiss(toast.id); }}>{action.label}</Button>
    {/if}
  </div>

  <IconButton icon={X} label="Dismiss notification" size="sm" onclick={() => ondismiss(toast.id)} />
</div>

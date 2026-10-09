<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Info from '@lucide/svelte/icons/info';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import type { Notice } from '$lib/ipc/bindings/Notice';
  import type { NoticeLevel } from '$lib/ipc/bindings/NoticeLevel';

  import type { IconComponent } from './icon';

  interface Props {
    notices: Notice[];
  }

  let { notices }: Props = $props();

  const LOOKS: Record<NoticeLevel, { icon: IconComponent; box: string; color: string }> = {
    info: { icon: Info, box: 'bg-info-wash', color: 'text-info' },
    warning: { icon: TriangleAlert, box: 'bg-warning-wash', color: 'text-warning' },
    error: { icon: CircleAlert, box: 'bg-danger-wash', color: 'text-danger' },
  };
</script>

{#if notices.length > 0}
  <ul class="flex flex-col gap-2" aria-label="Notices">
    {#each notices as notice, index (index)}
      {@const look = LOOKS[notice.level]}
      <li class="flex items-start gap-3 rounded-md border border-line px-3.5 py-2.5 text-sm text-fg {look.box}" role={notice.level === 'error' ? 'alert' : 'status'}>
        <look.icon class="mt-0.5 shrink-0 {look.color}" size={16} aria-hidden="true" />
        <span>{notice.message}</span>
      </li>
    {/each}
  </ul>
{/if}

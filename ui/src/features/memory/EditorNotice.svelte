<script lang="ts">
  import type { IconComponent } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import NoticeHead from '$lib/components/NoticeHead.svelte';
  import type { NoticeLevel } from '$lib/ipc/bindings/NoticeLevel';
  import { cx } from '$lib/utils/cx';

  interface Props {
    title?: string;
    body: string;
    level?: NoticeLevel;
    icon?: IconComponent;
  }

  let { title, body, level = 'warning', icon }: Props = $props();
</script>

<!-- Something about the item the editor was opened on that the user should know before saving. -->
<div class={cx('rounded-md p-3', NOTICE_LOOKS[level].box)} role="status">
  <NoticeHead {level} {title} {body} {icon} />
</div>

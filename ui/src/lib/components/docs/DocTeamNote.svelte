<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import { ICON } from '../icon';

  interface Props {
    /** How many columns the row spans. */
    span: number;
    /** Why the latest refresh of the team documents failed. */
    text: string;
  }

  let { span, text }: Props = $props();
</script>

<tr>
  <td colspan={span} class="border-b border-line/60 px-3 py-2.5 text-sm text-fg-2">
    <p class="flex items-start gap-2" role="status"><CircleAlert size={ICON.sm} aria-hidden="true" class="mt-0.5 shrink-0 text-warning" /><span class="min-w-0 break-words">{text}</span></p>
  </td>
</tr>

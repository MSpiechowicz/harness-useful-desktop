<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** The harness whose chat is a preview, named in the tooltip. */
    name: string;
  }

  let { name }: Props = $props();

  const tooltip = $derived(t('chat.preview.tooltip', { harness: name }));
</script>

<!-- Beside the harness mark in the chat header, so it is the height of a control. The tooltip text is also in the page for
     screen readers, since the badge is not something to tab to. -->
<Tooltip text={tooltip} side="bottom">
  {#snippet children()}
    <Badge tone="info" class="h-control rounded-md! px-2.5 text-sm!">
      {t('chat.preview.label')}
      <span class="sr-only">{tooltip}</span>
    </Badge>
  {/snippet}
</Tooltip>

<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Download from '@lucide/svelte/icons/download';
  import Lock from '@lucide/svelte/icons/lock';
  import Replace from '@lucide/svelte/icons/replace';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeReplace } from '$lib/stores/knowledgeReplace.svelte';
  import { uid } from '$lib/utils/ids';

  interface Props {
    /** The document has a pending update beside its live version: Download then offers each. */
    update: boolean;
    /** The user may not change the document (a read-only role on the remote place): Replace and Delete are off, with the reason under them. */
    readOnly: boolean;
    /** Replace file… is offered here. Off while the Review section offers it (changes were requested of the author). */
    replace?: boolean;
    ondownload: (live: boolean) => void;
    onreplace: () => void;
    ondelete: () => void;
  }

  let { update, readOnly, replace = true, ondownload, onreplace, ondelete }: Props = $props();

  const readOnlyId = uid('knowledge-read-only');

</script>

<!-- Three buttons with a 16px icon each, at their own width (they never shrink and squeeze the icons). They fit one row at the
     panel's width. A panel dragged narrower moves Delete to its own line under the other two. -->
<div>
  <div class="flex flex-wrap items-center gap-2 *:shrink-0">
    {#if update}
      <Popover label={t('knowledge.drafts.actions.download')} kind="menu" align="start" class="w-64">
        {#snippet trigger({ props })}
          <Button {...props}>
            {#snippet leading()}<Download size={ICON.sm} aria-hidden="true" />{/snippet}
            {t('knowledge.drafts.actions.download')}
            {#snippet trailing()}<ChevronDown size={ICON.sm} aria-hidden="true" />{/snippet}
          </Button>
        {/snippet}

        {#snippet children({ close })}
          <MenuItem onclick={() => { close(); ondownload(false); }}>{t('knowledge.drafts.actions.downloadUpdate')}</MenuItem>
          <MenuItem onclick={() => { close(); ondownload(true); }}>{t('knowledge.drafts.actions.downloadLive')}</MenuItem>
        {/snippet}
      </Popover>
    {:else}
      <Button onclick={() => ondownload(false)}>
        {#snippet leading()}<Download size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('knowledge.drafts.actions.download')}
      </Button>
    {/if}

    {#if replace}
      <Button disabled={readOnly} loading={knowledgeReplace.busy || knowledgeReplace.picking} aria-describedby={readOnly ? readOnlyId : undefined} onclick={onreplace}>
        {#snippet leading()}<Replace size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('knowledge.panel.replace')}
      </Button>
    {/if}

    <Button variant="dangerQuiet" aria-label={t('knowledge.panel.delete')} disabled={readOnly} aria-describedby={readOnly ? readOnlyId : undefined} onclick={ondelete}>
      {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('knowledge.panel.deleteShort')}
    </Button>
  </div>

  {#if readOnly}
    <p id={readOnlyId} in:rise={{ y: 4, duration: 200 }} class="mt-3 flex items-center gap-2 text-sm text-fg-3"><Lock size={ICON.sm} class="shrink-0" aria-hidden="true" />{t('knowledge.panel.readOnly')}</p>
  {/if}
</div>

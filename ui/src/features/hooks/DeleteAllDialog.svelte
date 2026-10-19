<script lang="ts">
  import Webhook from '@lucide/svelte/icons/webhook';

  import DocDeleteAllDialog from '$lib/components/docs/DocDeleteAllDialog.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { hooks } from './hooks-data.svelte';

  interface Props {
    open: boolean;
    /** How many hooks the list holds: the number the question names. */
    count: number;
    /** The hooks are gone: whatever was open on one lets go of it. */
    onremoved: () => void;
  }

  let { open = $bindable(false), count, onremoved }: Props = $props();
</script>

<DocDeleteAllDialog
  bind:open
  {count}
  {onremoved}
  icon={Webhook}
  title={(asked) => tn('hooks.deleteAll.title', asked)}
  subject={(asked) => tn('hooks.count', asked)}
  description={t('hooks.deleteAll.body')}
  confirmLabel={t('hooks.deleteAll.confirm')}
  cancelLabel={t('hooks.deleteAll.cancel')}
  failed={t('hooks.deleteAll.failed')}
  remove={() => hooks.removeAll(project.active?.path ?? null)}
/>

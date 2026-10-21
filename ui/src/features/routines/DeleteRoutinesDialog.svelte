<script lang="ts">
  import CalendarClock from '@lucide/svelte/icons/calendar-clock';
  import Users from '@lucide/svelte/icons/users';

  import DocDeleteAllDialog from '$lib/components/docs/DocDeleteAllDialog.svelte';
  import { type DeleteReach, deleteScopeText } from '$lib/components/docs/doc-scope';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { project } from '$lib/stores/project.svelte';

  import { routines } from './routines-data.svelte';

  interface Props {
    open: boolean;
    /** `all`: the routines of the global scope and the open project. `mine`: the routines this user shared with the team. */
    kind: 'all' | 'mine';
    /** How many routines the question names (not used for `mine`: nobody can say which of the team's routines are the user's). */
    count: number;
    /** What goes with the global routines: the open project's, by the path the delete is sent with. */
    reach: DeleteReach;
    /** The routines are gone: an open panel of one is closed. */
    onremoved: () => void;
  }

  let { open = $bindable(false), kind, count, reach, onremoved }: Props = $props();
</script>

<DocDeleteAllDialog
  bind:open
  {count}
  {onremoved}
  icon={kind === 'all' ? CalendarClock : Users}
  title={(asked) => (kind === 'all' ? tn('routines.deleteAll.title', asked, { where: deleteScopeText(reach.project) }) : t('routines.removeMine.title'))}
  subject={(asked) => (kind === 'all' ? tn('routines.count', asked) : t('routines.removeMine.subject'))}
  description={t(kind === 'all' ? 'routines.deleteAll.body' : 'routines.removeMine.body')}
  confirmLabel={t(kind === 'all' ? 'routines.deleteAll.confirm' : 'routines.removeMine.confirm')}
  cancelLabel={t('routines.deleteAll.cancel')}
  failed={t(kind === 'all' ? 'routines.deleteAll.failed' : 'routines.removeMine.failed')}
  remove={() => (kind === 'all' ? routines.removeAll(project.active?.path ?? null) : routines.removeMine())}
/>

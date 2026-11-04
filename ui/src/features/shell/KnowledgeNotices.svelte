<script lang="ts">
  import { untrack } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';

  /** How long the notice about a database that waits for a teammate stays: nothing here can be done about it. */
  const WAITS_MS = 15_000;

  /** The toast that asks to restart harnesses, while it is on screen. */
  let restartToast: number | null = null;
  /** The toast about a database that waits for a teammate's update, shown once for as long as that is so. */
  let waitsShown = false;

  // The database was moved to a newer version by this computer's update: running harnesses still use the old one until they restart.
  // The toast stays until the user closes it, and closing it ends the notice for good, as the backend remembers it.
  $effect(() => {
    const restart = knowledgeChanges.pulse?.restartHarnesses === true;

    untrack(() => {
      if (restart && restartToast === null) {
        restartToast = toasts.push({
          title: t('shell.notice.knowledgeRestart.title'),
          description: t('shell.notice.knowledgeRestart.body'),
          duration: 0,
          action: { label: t('shell.notice.knowledgeRestart.action'), run: () => undefined },
        });
      } else if (!restart && restartToast !== null) {
        // Dismissed on another path (another window): the notice goes away with it.
        toasts.dismiss(restartToast);
        restartToast = null;
      }
    });
  });

  // The toast went away, by its button or its close control: the notice is done.
  $effect(() => {
    const open = toasts.items.some((item) => item.id === restartToast);

    untrack(() => {
      if (restartToast !== null && !open) {
        restartToast = null;
        void ipc.invoke('knowledge_restart_notice_dismiss', undefined).catch(() => {
          // The notice comes back at the next launch, and nothing else is lost.
        });
      }
    });
  });

  // A read-only role cannot upgrade the shared database: say so once, then stay quiet until it is true again.
  $effect(() => {
    const waits = knowledgeChanges.pulse?.upgradeWaits === true;

    untrack(() => {
      if (waits && !waitsShown) {
        waitsShown = true;
        toasts.push({ title: t('shell.notice.knowledgeWaits.title'), description: t('shell.notice.knowledgeWaits.body'), tone: 'warning', duration: WAITS_MS });
      } else if (!waits) {
        waitsShown = false;
      }
    });
  });
</script>

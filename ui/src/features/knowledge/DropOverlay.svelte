<script lang="ts">
  import { onMount } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
  import { knowledgeReplace } from '$lib/stores/knowledgeReplace.svelte';

  import DropVeil from './DropVeil.svelte';

  let hovering = $state(false);

  // Files dragged in from the file manager are native drops (see `drop_listen`): the shell keeps their paths and says when they arrive, and the
  // import takes them from there. The webview gets no drag events for them, so the shell also says where the pointer is. The window's own drag events cover a browser, where nothing names a path.
  onMount(() => {
    let listening = true;

    ipc
      .invoke('drop_listen', {
        onDrop: (event) => {
          if (!listening) {
            return;
          }

          if (event.type === 'hover') {
            hovering = event.active;
            knowledgeReplace.nativeHover(event.active, event.at);
          } else {
            hovering = false;

            // A single file dropped on the panel of the open document replaces it. Everything else is an import.
            if (!knowledgeReplace.nativeDrop(event.at)) {
              void knowledgeImport.takeDrop();
            }
          }
        },
      })
      .catch(() => {});

    return () => {
      listening = false;
    };
  });

  const carriesFiles = (event: DragEvent): boolean => event.dataTransfer?.types.includes('Files') ?? false;

  function onDragEnter(event: DragEvent): void {
    if (carriesFiles(event)) {
      event.preventDefault();
      hovering = true;
    }
  }

  function onDragOver(event: DragEvent): void {
    if (carriesFiles(event)) {
      event.preventDefault();
    }
  }

  function onDragLeave(event: DragEvent): void {
    // A drag that leaves the window has no element it goes to.
    if (event.relatedTarget === null) {
      hovering = false;
    }
  }

  function onDrop(event: DragEvent): void {
    if (carriesFiles(event)) {
      // A panel that took the drop (to replace its document) has prevented the default already: the drop is not for an import then.
      const taken = event.defaultPrevented;

      event.preventDefault();
      hovering = false;

      if (!taken) {
        void knowledgeImport.takeDrop();
      }
    }
  }
</script>

<svelte:window ondragenter={onDragEnter} ondragover={onDragOver} ondragleave={onDragLeave} ondrop={onDrop} />

<!-- A file dragged over the panel of an open document is for replacing it: the panel says so and takes the drop. -->
<DropVeil shown={hovering && !knowledgeReplace.overPanel} title={t('knowledge.drop.title')} hint={t('knowledge.drop.hint')} />

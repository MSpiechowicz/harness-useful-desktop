<script lang="ts">
  import Upload from '@lucide/svelte/icons/upload';
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';

  import { ICON } from '$lib/components/icon';
  import { DURATION, rise, scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';

  let hovering = $state(false);

  // Files dragged in from the file manager are native drops (see `drop_listen`): the shell keeps their paths and says when they arrive, and the
  // import takes them from there. The window's own drag events cover a browser, where nothing names a path.
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
          } else {
            hovering = false;
            void knowledgeImport.takeDrop();
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
      event.preventDefault();
      hovering = false;
      void knowledgeImport.takeDrop();
    }
  }
</script>

<svelte:window ondragenter={onDragEnter} ondragover={onDragOver} ondragleave={onDragLeave} ondrop={onDrop} />

{#if hovering}
  <div in:fade={{ duration: scaled(DURATION.base) }} out:fade={{ duration: scaled(DURATION.fast) }} class="pointer-events-none absolute inset-0 z-30 grid place-items-center bg-surface-1/85 p-6 backdrop-blur-sm" aria-hidden="true">
    <div in:rise={{ y: 10, duration: 280 }} class="flex size-full flex-col items-center justify-center gap-4 rounded-xl border-2 border-dashed border-accent/50 bg-accent-wash text-center">
      <span class="grid size-16 place-items-center rounded-lg border border-line-strong bg-surface-2 text-accent-text shadow-float"><Upload size={28} strokeWidth={1.6} /></span>

      <div class="flex flex-col gap-1">
        <p class="text-xl text-fg">{t('knowledge.drop.title')}</p>
        <p class="text-base text-fg-2">{t('knowledge.drop.hint')}</p>
      </div>
    </div>
  </div>
{/if}

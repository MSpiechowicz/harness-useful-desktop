<script lang="ts">
  import X from '@lucide/svelte/icons/x';
  import { expoOut } from 'svelte/easing';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { DURATION, scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
  import { noteList } from '$lib/stores/memoryNotes.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { scriptList } from '$lib/stores/memoryScripts.svelte';

  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import { discardLocal } from './notes-discard';
  import { COPY_TEXT } from './notes-copy-text';
  import { strandedNotes, strandedScripts, teamOf } from './notes-team.svelte';

  interface Props {
    /** What is stranded: the notes or the scripts saved on this computer. */
    kind?: MemoryCopyKind;
    /** Opens the dialog that copies them to the remote database. */
    oncopy: () => void;
  }

  let { kind = 'notes', oncopy }: Props = $props();

  const look = NOTICE_LOOKS.info;

  let confirmer: SessionConfirm | undefined = $state();

  const team = $derived(teamOf(kind));
  const stranded = $derived(kind === 'notes' ? strandedNotes(memoryRemote.value, noteList.data) : strandedScripts(memoryRemote.value, scriptList.data));
  // The banner is dismissed at a count: it comes back when more are stranded than that.
  const shown = $derived(stranded !== null && stranded > 0 && stranded > team.dismissedAt);

  // The settings are read when the view opens, as another tool may have changed the routing since they were last read.
  onMount(() => {
    void memoryRemote.load();
  });

  $effect(() => {
    if (stranded !== null) {
      team.follow(stranded);
    }
  });

  function remove(): void {
    if (confirmer && stranded) {
      void discardLocal(kind, { all: stranded }, (request) => confirmer!.ask(request));
    }
  }
</script>

{#if shown && stranded !== null}
  <div transition:slide={{ duration: scaled(DURATION.slow), easing: expoOut }} class="shrink-0 overflow-hidden">
    <!-- Entries saved on this computer are invisible while their scope reads the remote database. This says so, and offers the two ways out. -->
    <div class="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-line px-4 py-2 {look.box}" role="status">
      <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
      <p class="min-w-0 flex-1 text-sm text-fg">{tn(COPY_TEXT[kind].bannerText, stranded)}</p>

      <div class="flex items-center gap-2">
        <Button variant="primary" onclick={oncopy}>{t('memory.copy.banner.copy')}</Button>
        <Button onclick={remove}>{t('memory.copy.banner.delete')}</Button>
        <IconButton icon={X} label={t('memory.copy.banner.dismiss')} onclick={() => team.dismiss(stranded)} />
      </div>
    </div>
  </div>
{/if}

<SessionConfirm bind:this={confirmer} />

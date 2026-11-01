<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { t, tn, type MessageKey } from '$lib/i18n/i18n.svelte';

  import type { RoutingConfirmCopy, RoutingEffect, RoutingScope } from './routing-change';

  interface Props {
    open?: boolean;
    /** What saving changes, per scope of notes and for the scripts. */
    effects: RoutingEffect<string>[];
    onconfirm: () => void;
    /** What the dialog says: the memory's own words unless given. */
    copy?: RoutingConfirmCopy;
  }

  let { open = $bindable(false), effects, onconfirm, copy }: Props = $props();

  /** The plural families of the sentence about the entries this computer holds, per scope (see `tn`). */
  const HIDE_LOCAL = {
    repository: 'settings.memoryRemote.change.hideLocal.repository',
    global: 'settings.memoryRemote.change.hideLocal.global',
    scripts: 'settings.memoryRemote.change.scripts.hideLocal',
  } as const;

  const TITLES = {
    repository: 'settings.memoryRemote.repositoryNotes.title',
    global: 'settings.memoryRemote.globalNotes.title',
    scripts: 'settings.memoryRemote.scripts.title',
  } as const satisfies Record<RoutingScope, MessageKey>;

  /** The scripts say "scripts" where the notes say "notes": the sentences differ only in that word, so each has its own. */
  function sentence(effect: RoutingEffect<RoutingScope>): string {
    const scripts = effect.scope === 'scripts';

    if (effect.kind === 'hideTeam') {
      return t(scripts ? 'settings.memoryRemote.change.scripts.hideTeam' : 'settings.memoryRemote.change.hideTeam');
    }

    if (effect.kind === 'showTeam') {
      return t(scripts ? 'settings.memoryRemote.change.scripts.showTeam' : 'settings.memoryRemote.change.showTeam');
    }

    if (effect.kind === 'saveHere') {
      return t(scripts ? 'settings.memoryRemote.change.scripts.saveHere' : 'settings.memoryRemote.change.saveHere');
    }

    if (effect.count === null) {
      return t(scripts ? 'settings.memoryRemote.change.scripts.hideLocal.unknown' : 'settings.memoryRemote.change.hideLocal.unknown');
    }

    return tn(HIDE_LOCAL[effect.scope], effect.count);
  }

  function memoryWords(effect: RoutingEffect<string>): { title: string; text: string } {
    const scoped = effect as RoutingEffect<RoutingScope>;

    return { title: t(TITLES[scoped.scope]), text: sentence(scoped) };
  }

  const words = $derived(copy ?? { title: t('settings.memoryRemote.change.title'), description: t('settings.memoryRemote.change.description'), describe: memoryWords });

  function confirm(): void {
    open = false;
    onconfirm();
  }
</script>

<Dialog bind:open title={words.title} description={words.description}>
  {#snippet children()}
    <ul class="flex flex-col gap-4">
      {#each effects as effect (effect.scope)}
        {@const said = words.describe(effect)}
        <li class="flex flex-col gap-0.5">
          <p class="text-base font-medium text-fg">{said.title}</p>
          <p class="text-sm text-fg-2">{said.text}</p>
        </li>
      {/each}
    </ul>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{t('settings.memoryRemote.change.cancel')}</Button>
    <Button variant="primary" onclick={confirm}>{t('settings.memoryRemote.save')}</Button>
  {/snippet}
</Dialog>

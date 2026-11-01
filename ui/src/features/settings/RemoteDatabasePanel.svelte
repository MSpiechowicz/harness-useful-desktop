<script lang="ts" generics="Kind extends string">
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import DatabasePicker from './DatabasePicker.svelte';
  import { REMOTE_SUBHEADING } from './remote-database';
  import type { RemoteConnectionDraft } from './remote-connection.svelte';
  import SecretField from './SecretField.svelte';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    /** The connection the user is editing; the owner keeps it to save what it holds. */
    draft: RemoteConnectionDraft<Kind>;
    /** The owner has other unsaved changes, so Save is on even when the connection is as saved. */
    otherChanges?: boolean;
    /** The chosen database cannot be tested or saved (the installed server does not know it). */
    blocked?: boolean;
    onsave: () => void;
    /** Notes under the URL field, given the id the URL field is described by. */
    notes?: Snippet<[string]>;
    /** More sections of the card between the connection and the buttons. */
    sections?: Snippet;
  }

  let { draft, otherChanges = false, blocked = false, onsave, notes, sections }: Props = $props();

  const config = $derived(draft.config);
  const copy = $derived(config.copy);

  const base = uid('remote-database');
  const connectionHeading = `${base}-connection`;
  const urlHint = `${base}-url`;
  const noteHint = `${base}-note`;

  const canTest = $derived(draft.busy === null && draft.urlUsable && !blocked);
  const canSave = $derived(canTest && (draft.connectionChanged || otherChanges));
  const problem = $derived(draft.saved.problem);
  const showInvalid = $derived(draft.urlInvalid && draft.touched);
</script>

<div class="divide-y divide-line">
  <section aria-labelledby={connectionHeading}>
    <h3 id={connectionHeading} class={REMOTE_SUBHEADING}>{t(copy.connectionHeading)}</h3>
    <div class="divide-y divide-line">
      <SettingRow title={t(copy.databaseTitle)} description={t(copy.databaseDescription)} points={[t(copy.restart), t(copy.stored, { path: draft.saved.configPath })]}>
        {#snippet control()}
          <DatabasePicker {config} value={draft.kind} onchange={(next) => draft.chooseKind(next)} />
        {/snippet}
      </SettingRow>

      <div class="flex flex-col gap-4 py-4">
        <div class="flex flex-col gap-1.5">
          <p class="text-base font-medium text-fg">{t(copy.urlLabel)}</p>
          <SecretField
            bind:value={() => draft.url, (next) => draft.typeUrl(next)}
            label={t(copy.urlLabel)}
            placeholder={draft.savedUrlStands ? t(copy.urlKeep) : t(config.urlPlaceholders[draft.kind])}
            showLabel={t(copy.urlShow)}
            hideLabel={t(copy.urlHide)}
            invalid={showInvalid}
            describedby={`${urlHint} ${noteHint}`}
            onblur={() => draft.blurUrl()}
          />
          {#if showInvalid}
            <p id={urlHint} class="text-xs text-danger" role="alert">{t(config.urlProblems[draft.kind])}</p>
          {:else if draft.savedUrlStands}
            <p id={urlHint} class="text-xs text-fg-3">{t(copy.urlSaved, { url: draft.saved.urlRedacted ?? '' })}</p>
          {/if}
          {@render notes?.(noteHint)}
        </div>

        {#if draft.takesToken}
          <div class="flex flex-col gap-1.5">
            <p class="text-base font-medium text-fg">{t(copy.tokenLabel)}</p>
            <SecretField
              bind:value={draft.token}
              label={t(copy.tokenLabel)}
              placeholder={draft.sameKind && draft.saved.hasToken ? t(copy.tokenKeep) : t(copy.tokenPlaceholder)}
              showLabel={t(copy.tokenShow)}
              hideLabel={t(copy.tokenHide)}
            />
          </div>
        {/if}
      </div>
    </div>
  </section>

  {@render sections?.()}

  <div class="flex flex-col gap-3 py-4">
    <div class="flex flex-wrap items-center gap-2">
      <Button loading={draft.busy === 'test'} disabled={!canTest} onclick={() => void draft.test()}>{t(copy.test)}</Button>
      <Button variant="primary" loading={draft.busy === 'save'} disabled={!canSave} onclick={onsave}>{t(copy.save)}</Button>
    </div>

    <div role="status">
      {#if draft.status}
        <p class={cx('text-sm', draft.status.ok ? 'text-success' : 'text-danger')}>{draft.status.text}</p>
      {/if}
    </div>

    {#if problem}
      <p class="text-sm text-danger" role="alert">{t(copy.problem, { problem })}</p>
    {/if}
  </div>
</div>

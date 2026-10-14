<script lang="ts">
  import { autofocus } from '$lib/a11y/autofocus';
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { StripOption } from '$lib/components/strip';
  import type { InstallScope } from '$lib/ipc/bindings/InstallScope';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { uid } from '$lib/utils/ids';

  import { actionRequest, targetProblem, type ActionRequest, type ComposeKind } from './action-model';

  interface Props {
    kind: ComposeKind;
    onsubmit: (request: ActionRequest) => void;
    onclose: () => void;
  }

  let { kind, onsubmit, onclose }: Props = $props();

  // The texts follow the language on screen; the placeholders are examples of what to type and stay as written.
  const COPY = {
    plugin: {
      get title() {
        return t('extensions.compose.plugin.title');
      },
      get description() {
        return t('extensions.compose.plugin.description');
      },
      get label() {
        return t('extensions.compose.plugin.label');
      },
      placeholder: 'plugin-name@marketplace',
      get hint() {
        return t('extensions.compose.plugin.hint');
      },
      scoped: true,
    },
    marketplace: {
      get title() {
        return t('extensions.compose.marketplace.title');
      },
      get description() {
        return t('extensions.compose.marketplace.description');
      },
      get label() {
        return t('extensions.compose.marketplace.label');
      },
      placeholder: 'owner/repository',
      get hint() {
        return t('extensions.compose.marketplace.hint');
      },
      scoped: false,
    },
    package: {
      get title() {
        return t('extensions.compose.package.title');
      },
      get description() {
        return t('extensions.compose.package.description');
      },
      get label() {
        return t('extensions.compose.package.label');
      },
      placeholder: 'npm:package-name',
      get hint() {
        return t('extensions.compose.package.hint');
      },
      scoped: true,
    },
  };

  const copy = $derived(COPY[kind]);
  const hintId = uid('install-hint');
  const projectPath = $derived(project.active?.path ?? null);
  const scopes = $derived<StripOption<InstallScope>[]>([
    { value: 'user', label: t('extensions.compose.forMe') },
    { value: 'project', label: project.active ? t('extensions.compose.thisProjectNamed', { name: project.active.name }) : t('extensions.compose.thisProject'), disabled: projectPath === null },
  ]);

  let value = $state('');
  let scope = $state<InstallScope>('user');
  // An empty field is not an error until the user tries to continue; leaving it half-typed is.
  let touched = $state(false);

  const problem = $derived(targetProblem(value));
  const shownProblem = $derived(touched && problem ? problem : null);

  function submit(): void {
    touched = true;

    const active = harness.active;
    if (problem || !active) {
      return;
    }

    const text = value.trim();
    const action =
      kind === 'plugin'
        ? ({ type: 'pluginInstall', target: text, scope } as const)
        : kind === 'marketplace'
          ? ({ type: 'marketplaceAdd', source: text } as const)
          : ({ type: 'packageInstall', source: text } as const);

    onsubmit(actionRequest(active.info.id, action, copy.scoped ? scope : 'user', projectPath));
  }
</script>

<Dialog open title={copy.title} description={copy.description} {onclose}>
  <form
    class="flex flex-col gap-4"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <label class="flex flex-col gap-1.5 text-sm text-fg-2">
      {copy.label}
      <input
        bind:value
        use:autofocus
        placeholder={copy.placeholder}
        autocomplete="off"
        spellcheck="false"
        aria-describedby={hintId}
        aria-invalid={shownProblem ? 'true' : undefined}
        onblur={() => (touched ||= value.trim() !== '')}
        class="h-control w-full rounded-md border bg-surface-2 px-3 font-mono text-base text-fg outline-none focus:border-focus {shownProblem ? 'border-danger' : 'border-line-strong'}"
      />
      <span id={hintId} class="text-xs {shownProblem ? 'text-danger' : 'text-fg-3'}" role={shownProblem ? 'alert' : undefined}>{shownProblem ?? copy.hint}</span>
    </label>

    {#if copy.scoped}
      <div class="flex flex-col gap-1.5 text-sm text-fg-2">
        <span>{t('extensions.compose.installFor')}</span>
        <SegmentedControl label={t('extensions.compose.installFor')} options={scopes} bind:value={() => scope, (next) => (scope = next)} />
        {#if projectPath === null}<span class="text-xs text-fg-3">{t('extensions.compose.openFirst')}</span>{/if}
      </div>
    {/if}

    <!-- Enter in the field submits through this hidden button. -->
    <button type="submit" class="hidden" tabindex="-1" aria-hidden="true">{t('extensions.compose.review')}</button>
  </form>

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t('extensions.compose.cancel')}</Button>
    <Button variant="primary" disabled={problem !== null && touched} onclick={submit}>{t('extensions.compose.review')}</Button>
  {/snippet}
</Dialog>

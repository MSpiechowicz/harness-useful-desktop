<script lang="ts">
  import { tick } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { ScriptRunRequest } from '$lib/ipc/bindings/ScriptRunRequest';
  import { scriptRequests } from '$lib/stores/scriptRequests.svelte';

  import RequestCountdown from './RequestCountdown.svelte';
  import ScriptRequestBody from './ScriptRequestBody.svelte';
  import { spelledOut } from './script-format';

  /** Run stays disabled for this long after a request appears: a double click or a held Enter meant for the last question must not approve code nobody read. */
  const ARM_MS = 500;
  const TICK_MS = 250;
  /** The safe answer takes the focus, so Enter on a freshly shown request declines it. */
  const SAFE_CHOICE = '[data-script-deny]';

  const current = $derived(scriptRequests.current);

  // The request stays on screen while the dialog fades out after the last answer.
  let remembered = $state.raw<ScriptRunRequest | null>(null);
  const request = $derived(current ?? remembered);

  let now = $state(Date.now());
  let armed = $state(false);

  const expired = $derived(scriptRequests.deadline - now <= 0);

  $effect(() => {
    if (current) {
      remembered = current;
    }
  });

  $effect(() => {
    if (!current) {
      return;
    }

    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), TICK_MS);

    return () => clearInterval(timer);
  });

  $effect(() => {
    armed = false;

    if (!current) {
      return;
    }

    const timer = setTimeout(() => (armed = true), ARM_MS);

    return () => clearTimeout(timer);
  });

  // The next request takes over the dialog, and with it the focus: it goes back to the safe choice.
  $effect(() => {
    if (current) {
      void tick().then(() => document.querySelector<HTMLElement>(SAFE_CHOICE)?.focus({ preventScroll: true }));
    }
  });

  function answer(allow: boolean): void {
    void scriptRequests.decide(allow);
  }
</script>

<!-- Closing the dialog any way at all (Escape, the scrim, the close button) is "Don't run": nothing runs unless it is chosen. -->
<Dialog
  bind:open={() => current !== null, () => {}}
  title={t('memory.request.title', { name: spelledOut(request?.script.name ?? '') })}
  description={t('memory.request.description', { client: request?.client ? spelledOut(request.client) : t('memory.request.unknownClient') })}
  size="lg"
  initialFocus={SAFE_CHOICE}
  onclose={() => answer(false)}
>
  {#snippet children()}
    {#if request}
      {#key request.requestId}
        <ScriptRequestBody {request} />
      {/key}
    {/if}
  {/snippet}

  {#snippet footer()}
    <div class="mr-auto flex h-control min-w-0 flex-col justify-center">
      <RequestCountdown deadline={scriptRequests.deadline} {now} />
      {#if scriptRequests.behind > 0}
        <p class="text-xs text-fg-3" role="status">{tn('memory.request.queue', scriptRequests.behind)}</p>
      {/if}
    </div>

    <Button data-script-deny onclick={() => answer(false)}>{t('memory.request.deny')}</Button>
    <Button variant="primary" disabled={!armed || expired || scriptRequests.answering} onclick={() => answer(true)}>{t('memory.request.run')}</Button>
  {/snippet}
</Dialog>

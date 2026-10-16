<script lang="ts">
  import MessageSquareDashed from '@lucide/svelte/icons/message-square-dashed';

  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav, type ViewId } from '$lib/stores/nav.svelte';

  import { VIEWS } from '../shell/views';

  interface Props {
    /** The installed harness the app cannot chat with yet. */
    name: string;
  }

  let { name }: Props = $props();

  /** What already works with such a harness, each one a way into its view. */
  const WORKS = $derived<{ id: ViewId; body: string }[]>([
    { id: 'sessions', body: t('chat.soon.sessions') },
    { id: 'config', body: t('chat.soon.config') },
    { id: 'extensions', body: t('chat.soon.extensions') },
  ]);

  const links = $derived(
    WORKS.flatMap((work) => {
      const view = VIEWS.find((candidate) => candidate.id === work.id);

      return view ? [{ ...work, label: view.label, icon: view.icon }] : [];
    }),
  );
</script>

<!-- The app reads and shows such a harness already, only the chat is still to come: say so, and lead to what works. -->
<div class="flex max-w-3xl flex-col items-center gap-8 px-8 py-14 text-center" role="status">
  <div in:rise={{ y: 10, duration: 360 }} class="relative grid size-16 place-items-center rounded-lg border border-line-strong bg-surface-2 shadow-float">
    <MessageSquareDashed class="text-accent-text" size={28} strokeWidth={1.6} aria-hidden="true" />
  </div>

  <div in:rise={{ y: 10, duration: 360, delay: 60 }} class="flex max-w-md flex-col gap-1.5">
    <h2 class="text-xl text-fg">{t('chat.soon.title', { harness: name })}</h2>
    <p class="text-base text-fg-2">{t('chat.soon.body', { harness: name })}</p>
  </div>

  <ul class="grid w-full grid-cols-1 gap-3 text-left sm:grid-cols-3" aria-label={t('chat.soon.works', { harness: name })}>
    {#each links as link, index (link.id)}
      <li in:rise={{ y: 10, duration: 360, delay: 120 + index * 50 }} class="flex">
        <button
          type="button"
          class="group flex w-full flex-col gap-3 rounded-lg border-card border-line bg-glass p-4 text-left transition-[border-color,box-shadow] duration-[var(--dur-base)] hover:edge-accent hover:ring-1 hover:ring-accent/30"
          onclick={() => nav.go(link.id)}
        >
          <span class="grid size-9 place-items-center rounded-md bg-accent-wash">
            <link.icon class="text-accent-text" size={18} strokeWidth={1.8} aria-hidden="true" />
          </span>
          <span class="text-base font-medium text-fg">{link.label}</span>
          <span class="text-sm text-fg-2">{link.body}</span>
        </button>
      </li>
    {/each}
  </ul>
</div>

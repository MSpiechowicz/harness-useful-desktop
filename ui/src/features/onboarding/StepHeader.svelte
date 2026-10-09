<script lang="ts">
  import { autofocus } from '$lib/a11y/autofocus';

  interface Props {
    eyebrow: string;
    title: string;
    body: string;
  }

  let { eyebrow, title, body }: Props = $props();
</script>

<header class="flex flex-col gap-2">
  <p class="text-sm font-medium tracking-wider text-accent-text uppercase">{eyebrow}</p>
  <h2 id="onboarding-title" tabindex="-1" use:autofocus class="text-3xl text-fg outline-none">{title}</h2>
  <p class="max-w-xl text-lg text-fg-2">{body}</p>
</header>

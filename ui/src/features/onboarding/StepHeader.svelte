<script lang="ts">
  import { autofocus } from '$lib/a11y/autofocus';

  interface Props {
    eyebrow: string;
    title: string;
    body: string;
  }

  let { eyebrow, title, body }: Props = $props();
</script>

<header class="flex flex-col gap-3 [@media(max-height:720px)]:gap-2">
  <p class="flex items-center gap-3 text-sm font-medium tracking-wider text-accent-text uppercase">
    <span class="h-px w-6 bg-accent/60" aria-hidden="true"></span>{eyebrow}
  </p>
  <h2 id="onboarding-title" tabindex="-1" use:autofocus class="text-[clamp(1.75rem,1.25rem+1.4vw,2.5rem)] leading-[1.1] tracking-tight text-balance text-fg outline-none">
    {title}
  </h2>
  <p class="max-w-xl text-lg text-pretty text-fg-2 [@media(max-height:720px)]:text-base">{body}</p>
</header>

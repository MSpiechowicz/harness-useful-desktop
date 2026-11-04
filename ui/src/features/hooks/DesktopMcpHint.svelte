<script lang="ts">
  import DocMcpHint from '$lib/components/docs/DocMcpHint.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { onboarding } from '../onboarding/onboarding-state.svelte';

  interface Props {
    /** Runs before the Recommended setup opens, e.g. to close the dialog this sits in. */
    onleave?: () => void;
    /** See `DocMcpHint`: off in a dialog. */
    centered?: boolean;
  }

  let { onleave, centered = true }: Props = $props();
</script>

<DocMcpHint message={t('hooks.mcpHint')} action={t('hooks.mcpHint.action')} {onleave} {centered} onsetup={() => onboarding.restart('setup')} />

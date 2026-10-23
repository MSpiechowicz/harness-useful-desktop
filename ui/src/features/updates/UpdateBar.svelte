<script lang="ts">
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { updates } from '$lib/stores/updates.svelte';
</script>

<!-- The update's progress bar: fills with the download, sweeps while the size or the install time is unknown. -->
<ProgressBar label={t('updates.progress.label')} fraction={updates.fraction} />

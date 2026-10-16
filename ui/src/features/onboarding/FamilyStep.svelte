<script lang="ts">
  import { onMount } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import CompanionCard from './CompanionCard.svelte';
  import { WIDTH } from './layout';
  import { setup } from './setup-state.svelte';
  import SetupStatus from './SetupStatus.svelte';
  import SkillsCard from './SkillsCard.svelte';
  import StepHeader from './StepHeader.svelte';

  // Opening the step reads the machine and nothing else; what the family would change is only listed here.
  onMount(() => {
    void setup.open();
  });
</script>

<div class={cx('mx-auto flex w-full flex-col gap-4 [@media(max-height:720px)]:gap-3', WIDTH.wide)}>
  <div class="mb-1 [@media(max-height:720px)]:mb-0">
    <StepHeader eyebrow={t('onboarding.family.eyebrow')} title={t('onboarding.family.title')} body={t('onboarding.family.body')} />
  </div>

  <SetupStatus placeholder="24rem">
    {#snippet children(data)}
      <!-- Every member in the same card, in the order the next step installs them. -->
      <CompanionCard companion="dashboard" server={data.dashboard} />
      <CompanionCard companion="memory" server={data.memory} />
      <SkillsCard harnesses={data.harnesses} />
    {/snippet}
  </SetupStatus>
</div>

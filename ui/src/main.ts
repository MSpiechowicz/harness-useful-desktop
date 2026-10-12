import { mount } from 'svelte';

import App from './App.svelte';
import './app.css';
import { i18n, toLang } from './lib/i18n/i18n.svelte';
import { language } from './lib/stores/language.svelte';
import { motion } from './lib/stores/motion.svelte';
import { prefs } from './lib/stores/prefs.svelte';
import { theme } from './lib/stores/theme.svelte';

/** The longest the first paint waits for the saved language; past it the app opens in English and switches when it arrives. */
const PREFS_WAIT_MS = 1500;

const target = document.getElementById('app');
if (!target) {
  throw new Error('Missing #app mount point');
}

/** Resolves when the preferences have loaded (or failed), or after the wait, whichever comes first. */
async function preferencesOrTimeout(): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, PREFS_WAIT_MS);
  });

  await Promise.race([prefs.load(), timeout]);
  clearTimeout(timer);
}

async function boot(root: HTMLElement): Promise<void> {
  // Mirror theme and motion onto <html> for the whole app lifetime, before the first render.
  theme.start();
  motion.start();

  // The saved language is loaded before the first paint, so a restart never flashes English.
  await preferencesOrTimeout();
  await i18n.load(toLang(prefs.value?.language));
  language.start();

  mount(App, { target: root });
}

void boot(target);

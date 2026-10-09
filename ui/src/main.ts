import { mount } from 'svelte';

import App from './App.svelte';
import './app.css';
import { motion } from './lib/stores/motion.svelte';
import { theme } from './lib/stores/theme.svelte';

const target = document.getElementById('app');
if (!target) {
  throw new Error('Missing #app mount point');
}

// Mirror theme and motion onto <html> for the whole app lifetime, before the first render.
theme.start();
motion.start();

mount(App, { target });

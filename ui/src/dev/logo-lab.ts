// Dev-only entry for `logo-lab.html` (served by `bun run dev:ui`/`dev:mock`, never part of the build).
import { mount } from 'svelte';

import '../app.css';
import { motion } from '../lib/stores/motion.svelte';
import LogoLab from './LogoLab.svelte';

const target = document.getElementById('lab');
if (!target) {
  throw new Error('Missing #lab mount point');
}

motion.start();

mount(LogoLab, { target });

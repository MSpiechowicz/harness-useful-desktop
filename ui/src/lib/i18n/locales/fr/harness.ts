import type { harness as enHarness } from '../en/harness';
import type { AreaMessages } from '../types';

export const harness: AreaMessages<typeof enHarness> = {
  'harness.boundary.setup': 'La configuration',
  'harness.boundary.intro': 'L’introduction',
  'harness.boundary.notifications': 'Les notifications',

  'harness.gate.noneTitle': 'Aucun harness trouvé',
  'harness.gate.noneBody': 'Harness Desktop fonctionne avec OMP et PI. Lancez la configuration pour les détecter.',
  'harness.gate.missingTitle': '{name} n’est pas installé',
  'harness.gate.setup': 'Configurer {name}',
  'harness.gate.retry': 'Réessayer',
  'harness.gate.banner': '{name} n’est pas installé ; affichage de ce qui se trouve sur le disque.',

  'harness.gate.config.missingBody': 'Installez-le, ou indiquez à Harness Desktop un exécutable existant, pour voir ici sa configuration.',
  'harness.gate.config.errorTitle': 'Impossible de lire la configuration',
  'harness.gate.config.loading': 'Chargement de la configuration',
  'harness.gate.config.refreshing': 'Actualisation de la configuration',
  'harness.gate.extensions.missingBody': 'Installez-le, ou indiquez à Harness Desktop un exécutable existant, pour voir ici ses extensions.',
  'harness.gate.extensions.errorTitle': 'Impossible de lire les extensions',
  'harness.gate.extensions.loading': 'Chargement des extensions',
  'harness.gate.extensions.refreshing': 'Actualisation des extensions',

  'harness.toast.scan': 'Impossible de rechercher les harnesses',
  'harness.toast.pickBinary': 'Impossible d’utiliser ce fichier',
  'harness.toast.clearBinary': 'Impossible de réinitialiser l’exécutable',
  'harness.toast.recents': 'Impossible de lire les projets récents',
  'harness.toast.pickFolder': 'Impossible d’ouvrir ce dossier',
  'harness.toast.openProject': 'Impossible d’ouvrir le projet',
  'harness.toast.closeProject': 'Impossible de fermer le projet',
  'harness.toast.forgetProject': 'Impossible d’oublier le projet',
};

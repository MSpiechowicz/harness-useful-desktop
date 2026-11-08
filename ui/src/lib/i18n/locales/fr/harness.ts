import type { harness as enHarness } from '../en/harness';
import type { AreaMessages } from '../types';

export const harness: AreaMessages<typeof enHarness> = {
  'harness.boundary.setup': 'La configuration',
  'harness.boundary.intro': 'L’introduction',
  'harness.boundary.notifications': 'Les notifications',

  'harness.gate.noneTitle': 'Aucun harness trouvé',
  'harness.gate.noneBody': 'Harness Desktop fonctionne avec OMP, PI, OpenCode, Claude Code et Codex. Lancez la configuration pour les détecter.',
  'harness.gate.missingTitle': '{name} n’est pas installé',
  'harness.gate.setup': 'Configurer {name}',
  'harness.gate.retry': 'Réessayer',
  'harness.gate.banner': '{name} n’est pas installé. Voici ce qui se trouve sur le disque.',

  'harness.gate.config.missingBody': 'Installez-le, ou indiquez à Harness Desktop un exécutable existant, pour voir ici sa configuration.',
  'harness.gate.config.errorTitle': 'Impossible de lire la configuration',
  'harness.gate.config.loading': 'Chargement de la configuration',
  'harness.gate.config.refreshing': 'Actualisation de la configuration',
  'harness.gate.extensions.missingBody': 'Installez-le, ou indiquez à Harness Desktop un exécutable existant, pour voir ici ses extensions.',
  'harness.gate.extensions.errorTitle': 'Impossible de lire les extensions',
  'harness.gate.extensions.loading': 'Chargement des extensions',
  'harness.gate.extensions.refreshing': 'Actualisation des extensions',

  'harness.switcher.label': 'Changer de harness',

  'harness.toast.pickBinary': 'Impossible d’utiliser ce fichier',
  'harness.toast.clearBinary': 'Impossible de réinitialiser l’exécutable',
  'harness.toast.recents': 'Impossible de lire les projets récents',
  'harness.toast.pickFolder': 'Impossible d’ouvrir ce dossier',
  'harness.toast.openProject': 'Impossible d’ouvrir le projet',
  'harness.toast.closeProject': 'Impossible de fermer le projet',
  'harness.toast.forgetProject': 'Impossible d’oublier le projet',

  'harness.update.offerTitle': '{name} v{version} est disponible',
  'harness.update.offerBody': 'Vous avez v{current}. Vous pouvez mettre à jour maintenant ou plus tard dans les Paramètres.',
  'harness.update.action': 'Mettre à jour',
  'harness.update.progressTitle': 'Mise à jour de {name}…',
  'harness.update.progressBody': 'Cela peut prendre quelques minutes. Les chats ouverts continuent de tourner.',
  'harness.update.doneTitle': '{name} mis à jour vers v{version}',
  'harness.update.doneBody': 'Les nouveaux chats utilisent la nouvelle version. Les chats ouverts gardent l’ancienne jusqu’à ce que vous les relanciez.',
  'harness.update.failedTitle': '{name} n’a pas été mis à jour',
  'harness.update.failedBody': 'Le programme de mise à jour n’a donné aucune raison.',
  'harness.update.copyCommand': 'Copier la commande',
  'harness.update.copied': 'La commande de mise à jour est dans le presse-papiers.',
};

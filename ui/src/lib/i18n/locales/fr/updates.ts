import type { updates as enUpdates } from '../en/updates';
import type { AreaMessages } from '../types';

export const updates: AreaMessages<typeof enUpdates> = {
  'updates.installerCloses': 'Le programme d’installation fermera Harness Desktop et le rouvrira une fois terminé.',
  'updates.restartStopsAgents': 'Les agents en cours s’arrêtent au redémarrage de l’application.',
  'updates.restartWarning.one': '{count} chat en cours sera arrêté, et toute réponse en cours sera perdue.',
  'updates.restartWarning.other': '{count} chats en cours seront arrêtés, et toute réponse en cours sera perdue.',

  'updates.unit.kb': 'Ko',
  'updates.unit.mb': 'Mo',
  'updates.progress.of': '{downloaded} sur {total}',

  'updates.failure.network.check': 'Impossible de joindre le serveur de mises à jour. Vérifiez votre connexion et réessayez.',
  'updates.failure.network.install': 'Le téléchargement a été interrompu. Rien n’a été modifié ; réessayez.',
  'updates.failure.verification': 'La mise à jour n’a pas pu être vérifiée et n’a pas été installée.',
  'updates.failure.manifest': 'Les informations de mise à jour étaient incomplètes ou ne contiennent rien pour ce système. Réessayez plus tard.',
  'updates.failure.installer': 'Le programme d’installation ne s’est pas terminé, ou la demande d’autorisation a été annulée. Rien n’a été modifié.',
  'updates.failure.busy': 'Une mise à jour est déjà en cours d’installation.',
  'updates.failure.stale': 'Cette mise à jour n’est plus disponible. Vérifiez à nouveau.',
  'updates.failure.unknown': 'Une erreur est survenue pendant la mise à jour. Rien n’a été modifié.',

  'updates.status.idle': 'Pas encore vérifié durant cette session.',
  'updates.status.checking': 'Recherche de mises à jour…',
  'updates.status.upToDate': 'Vous êtes à jour. Vérifié à {time}.',
  'updates.status.notConfigured': 'Cette version ne peut pas rechercher de mises à jour.',
  'updates.status.available': 'La version {version} est disponible.',
  'updates.status.downloading': 'Téléchargement de la version {version}…',
  'updates.status.installing': 'Installation de la version {version}…',
  'updates.status.readyToRelaunch': 'La version {version} est installée. Redémarrez pour terminer.',
  'updates.status.failedCheck': 'Impossible de rechercher les mises à jour.',
  'updates.status.failedInstall': 'La mise à jour n’a pas été installée.',


  'updates.install.restart': 'Installer et redémarrer',

  'updates.notice.label': 'Mise à jour',
  'updates.notice.available': 'La version {version} est disponible',
  'updates.notice.updating': 'Mise à jour vers la version {version}',
  'updates.notice.installed': 'La version {version} est installée',
  'updates.notice.failed': 'La mise à jour n’a pas été installée',
  'updates.notice.availableBody': 'Redémarrez après l’installation pour terminer',
  'updates.notice.restartBody': 'Redémarrez pour terminer. Les agents en cours s’arrêtent au redémarrage de l’application.',
  'updates.notice.install': 'Installer',
  'updates.notice.view': 'Voir',
  'updates.notice.later': 'Plus tard',
  'updates.notice.finish': 'Terminer la mise à jour',
  'updates.notice.restart': 'Redémarrer maintenant',
  'updates.notice.retry': 'Réessayer',
  'updates.notice.dismiss': 'Ignorer',

  'updates.progress.label': 'Progression de la mise à jour',
  'updates.progress.installing': 'Installation…',
  'updates.progress.downloading': 'Téléchargement…',

  'updates.restart.title': 'Redémarrer maintenant ?',
  'updates.restart.confirm': 'Redémarrer maintenant',
  'updates.restart.finish': 'Terminer la mise à jour',
  'updates.restart.keepWorking': 'Continuer à travailler',
  'updates.restart.failed': 'Impossible de redémarrer',
};

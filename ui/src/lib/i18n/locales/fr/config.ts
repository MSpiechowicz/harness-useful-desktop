import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Configuration',
  'config.reload': 'Recharger la configuration',
  'config.resize': 'Redimensionner la liste des fichiers',
  'config.empty': 'Aucun fichier de configuration trouvé pour ce harness.',

  'config.layer.user': 'Utilisateur',
  'config.layer.profile': 'Profil',
  'config.layer.project': 'Projet',
  'config.format.text': 'Texte',

  'config.list.label': 'Fichiers de configuration',
  'config.list.layer': 'Niveau « {layer} »',
  'config.list.error': 'Erreur',
  'config.list.notFound': 'Introuvable',

  'config.detail.reveal': 'Afficher dans le gestionnaire de fichiers',
  'config.detail.readError': 'Ce fichier n’a pas pu être lu',
  'config.detail.missingTitle': 'Pas encore de fichier',
  'config.detail.missingBody': 'Le harness n’a aucun réglage à cet emplacement. Les réglages de ce niveau apparaîtront dès que le fichier existera.',
  'config.detail.filter': 'Filtrer les clés et les valeurs',
  'config.detail.matches.one': '{count} résultat',
  'config.detail.matches.other': '{count} résultats',
  'config.detail.entries.one': '{count} entrée de premier niveau',
  'config.detail.entries.other': '{count} entrées de premier niveau',
  'config.detail.collapseAll': 'Tout réduire',
  'config.detail.expandAll': 'Tout développer',
  'config.detail.treeLabel': 'Réglages de {title}',
  'config.detail.secrets': 'Les secrets sont masqués ici et ne quittent jamais le backend.',
};

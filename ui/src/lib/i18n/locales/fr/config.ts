import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Configuration',
  'config.resize': 'Redimensionner la liste des fichiers',
  'config.drawer.label': 'Fichiers',
  'config.list.noMatch': 'Aucun fichier ne correspond à la recherche.',
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
  'config.detail.treeLabel': 'Réglages de {title}',
  'config.detail.secrets': 'Les secrets sont masqués ici et ne quittent jamais le backend.',
  'config.detail.aside': 'Détails du fichier',

  'config.search.label': 'Rechercher dans les clés et les valeurs',
  'config.search.placeholder': 'Rechercher dans les réglages',
  'config.search.matches.one': '{count} résultat',
  'config.search.matches.other': '{count} résultats',

  'config.mode.label': 'Affichage',
  'config.mode.structured': 'Structuré',
  'config.mode.source': 'Source',

  'config.menu.label': 'Plus d’actions',
  'config.menu.collapseAll': 'Tout réduire',
  'config.menu.expandAll': 'Tout développer',
  'config.menu.copyPath': 'Copier le chemin',

  'config.overview.title': 'Aperçu',
  'config.overview.entries': 'entrées de premier niveau',
  'config.overview.values': 'valeurs',
  'config.overview.depth': 'niveaux de profondeur',
  'config.overview.secrets': 'secrets masqués',

  'config.file.title': 'Fichier',
  'config.file.layer': 'Niveau',
  'config.file.format': 'Format',
  'config.file.profile': 'Profil',
  'config.file.path': 'Chemin',

  'config.profiles.title': 'Profils',
  'config.profiles.hint': 'Lancez {harness} avec --profile <name> pour en utiliser un.',
  'config.profiles.less': 'Afficher moins',
  'config.profiles.all': 'Afficher les {count}',
};

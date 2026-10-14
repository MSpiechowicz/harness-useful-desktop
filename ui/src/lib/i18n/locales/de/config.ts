import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Konfiguration',
  'config.resize': 'Größe der Dateiliste ändern',
  'config.empty': 'Für dieses Harness wurden keine Konfigurationsdateien gefunden.',

  'config.layer.user': 'Benutzer',
  'config.layer.profile': 'Profil',
  'config.layer.project': 'Projekt',
  'config.format.text': 'Text',

  'config.list.label': 'Konfigurationsdateien',
  'config.list.layer': 'Ebene „{layer}“',
  'config.list.error': 'Fehler',
  'config.list.notFound': 'Nicht gefunden',

  'config.detail.reveal': 'Im Dateimanager anzeigen',
  'config.detail.readError': 'Diese Datei konnte nicht gelesen werden',
  'config.detail.missingTitle': 'Noch keine Datei',
  'config.detail.missingBody': 'Das Harness hat unter diesem Pfad keine Einstellungen. Einstellungen dieser Ebene erscheinen, sobald die Datei existiert.',
  'config.detail.treeLabel': 'Einstellungen von {title}',
  'config.detail.secrets': 'Geheimnisse werden hier maskiert und verlassen nie das Backend.',
  'config.detail.aside': 'Dateidetails',

  'config.search.label': 'Schlüssel und Werte durchsuchen',
  'config.search.placeholder': 'Einstellungen durchsuchen',
  'config.search.matches.one': '{count} Treffer',
  'config.search.matches.other': '{count} Treffer',

  'config.mode.label': 'Ansicht',
  'config.mode.structured': 'Strukturiert',
  'config.mode.source': 'Quelltext',

  'config.menu.label': 'Weitere Aktionen',
  'config.menu.collapseAll': 'Alle einklappen',
  'config.menu.expandAll': 'Alle ausklappen',
  'config.menu.copyPath': 'Pfad kopieren',

  'config.overview.title': 'Überblick',
  'config.overview.entries': 'Einträge auf oberster Ebene',
  'config.overview.values': 'Werte',
  'config.overview.depth': 'Ebenen tief',
  'config.overview.secrets': 'Geheimnisse maskiert',

  'config.file.title': 'Datei',
  'config.file.layer': 'Ebene',
  'config.file.format': 'Format',
  'config.file.profile': 'Profil',
  'config.file.path': 'Pfad',

  'config.profiles.title': 'Profile',
  'config.profiles.hint': 'Starte {harness} mit --profile <name>, um eines zu verwenden.',
  'config.profiles.less': 'Weniger anzeigen',
  'config.profiles.all': 'Alle {count} anzeigen',
};

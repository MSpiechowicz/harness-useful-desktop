import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Konfiguration',
  'config.reload': 'Konfiguration neu laden',
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
  'config.detail.filter': 'Schlüssel und Werte filtern',
  'config.detail.matches.one': '{count} Treffer',
  'config.detail.matches.other': '{count} Treffer',
  'config.detail.entries.one': '{count} Eintrag auf oberster Ebene',
  'config.detail.entries.other': '{count} Einträge auf oberster Ebene',
  'config.detail.collapseAll': 'Alle einklappen',
  'config.detail.expandAll': 'Alle ausklappen',
  'config.detail.treeLabel': 'Einstellungen von {title}',
  'config.detail.secrets': 'Geheimnisse werden hier maskiert und verlassen nie das Backend.',
};

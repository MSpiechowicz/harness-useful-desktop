import type { updates as enUpdates } from '../en/updates';
import type { AreaMessages } from '../types';

export const updates: AreaMessages<typeof enUpdates> = {
  'updates.installerCloses': 'Das Installationsprogramm beendet Harness Desktop und öffnet es wieder, wenn es fertig ist.',
  'updates.restartStopsAgents': 'Laufende Agenten werden beendet, wenn die App neu startet.',
  'updates.restartWarning.one': '{count} laufender Chat wird beendet, und jede Antwort in Arbeit geht verloren.',
  'updates.restartWarning.other': '{count} laufende Chats werden beendet, und jede Antwort in Arbeit geht verloren.',

  'updates.unit.kb': 'KB',
  'updates.unit.mb': 'MB',
  'updates.progress.of': '{downloaded} von {total}',

  'updates.failure.network.check': 'Der Update-Server ist nicht erreichbar. Prüfe deine Verbindung und versuche es erneut.',
  'updates.failure.network.install': 'Der Download wurde unterbrochen. Es wurde nichts geändert; versuche es erneut.',
  'updates.failure.verification': 'Das Update konnte nicht verifiziert werden und wurde nicht installiert.',
  'updates.failure.manifest': 'Die Update-Informationen waren unvollständig oder enthalten nichts für dieses System. Versuche es später erneut.',
  'updates.failure.installer': 'Das Installationsprogramm wurde nicht fertig, oder die Berechtigungsabfrage wurde abgebrochen. Es wurde nichts geändert.',
  'updates.failure.busy': 'Ein Update wird bereits installiert.',
  'updates.failure.stale': 'Dieses Update ist nicht mehr verfügbar. Suche erneut.',
  'updates.failure.unknown': 'Beim Aktualisieren ist etwas schiefgegangen. Es wurde nichts geändert.',

  'updates.status.idle': 'In dieser Sitzung noch nicht geprüft.',
  'updates.status.checking': 'Suche nach Updates…',
  'updates.status.upToDate': 'Du bist auf dem neuesten Stand. Geprüft um {time}.',
  'updates.status.notConfigured': 'Dieser Build kann nicht nach Updates suchen.',
  'updates.status.available': 'Version {version} ist verfügbar.',
  'updates.status.downloading': 'Version {version} wird heruntergeladen…',
  'updates.status.installing': 'Version {version} wird installiert…',
  'updates.status.readyToRelaunch': 'Version {version} ist installiert. Starte neu, um abzuschließen.',
  'updates.status.failedCheck': 'Die Suche nach Updates ist fehlgeschlagen.',
  'updates.status.failedInstall': 'Das Update wurde nicht installiert.',


  'updates.install.restart': 'Installieren & neu starten',

  'updates.notice.label': 'Update',
  'updates.notice.available': 'Version {version} ist verfügbar',
  'updates.notice.updating': 'Aktualisierung auf {version}',
  'updates.notice.installed': 'Version {version} ist installiert',
  'updates.notice.failed': 'Das Update wurde nicht installiert',
  'updates.notice.availableBody': 'Nach der Installation neu starten, um abzuschließen',
  'updates.notice.restartBody': 'Starte neu, um abzuschließen. Laufende Agenten werden beendet, wenn die App neu startet.',
  'updates.notice.install': 'Installieren',
  'updates.notice.view': 'Ansehen',
  'updates.notice.later': 'Später',
  'updates.notice.finish': 'Update abschließen',
  'updates.notice.restart': 'Jetzt neu starten',
  'updates.notice.retry': 'Erneut versuchen',
  'updates.notice.dismiss': 'Schließen',

  'updates.progress.label': 'Update-Fortschritt',
  'updates.progress.installing': 'Installiere…',
  'updates.progress.downloading': 'Lade herunter…',

  'updates.restart.title': 'Jetzt neu starten?',
  'updates.restart.confirm': 'Jetzt neu starten',
  'updates.restart.finish': 'Update abschließen',
  'updates.restart.keepWorking': 'Weiterarbeiten',
  'updates.restart.failed': 'Neustart nicht möglich',
};

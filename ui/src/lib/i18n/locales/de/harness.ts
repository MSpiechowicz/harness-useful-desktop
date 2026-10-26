import type { harness as enHarness } from '../en/harness';
import type { AreaMessages } from '../types';

export const harness: AreaMessages<typeof enHarness> = {
  'harness.boundary.setup': 'Die Einrichtung',
  'harness.boundary.intro': 'Das Intro',
  'harness.boundary.notifications': 'Die Benachrichtigungen',

  'harness.gate.noneTitle': 'Kein Harness gefunden',
  'harness.gate.noneBody': 'Harness Desktop arbeitet mit OMP, PI, OpenCode, Claude Code und Codex. Starte die Einrichtung, um sie zu erkennen.',
  'harness.gate.missingTitle': '{name} ist nicht installiert',
  'harness.gate.setup': '{name} einrichten',
  'harness.gate.retry': 'Erneut versuchen',
  'harness.gate.banner': '{name} ist nicht installiert. Angezeigt wird, was auf der Festplatte liegt.',

  'harness.gate.config.missingBody': 'Installiere es oder verweise Harness Desktop auf ein vorhandenes Programm, um hier die Konfiguration zu sehen.',
  'harness.gate.config.errorTitle': 'Die Konfiguration konnte nicht gelesen werden',
  'harness.gate.config.loading': 'Konfiguration wird geladen',
  'harness.gate.config.refreshing': 'Konfiguration wird aktualisiert',
  'harness.gate.extensions.missingBody': 'Installiere es oder verweise Harness Desktop auf ein vorhandenes Programm, um hier die Erweiterungen zu sehen.',
  'harness.gate.extensions.errorTitle': 'Die Erweiterungen konnten nicht gelesen werden',
  'harness.gate.extensions.loading': 'Erweiterungen werden geladen',
  'harness.gate.extensions.refreshing': 'Erweiterungen werden aktualisiert',

  'harness.switcher.label': 'Harness wechseln',

  'harness.toast.pickBinary': 'Diese Datei lässt sich nicht verwenden',
  'harness.toast.clearBinary': 'Das Programm konnte nicht zurückgesetzt werden',
  'harness.toast.recents': 'Zuletzt verwendete Projekte konnten nicht gelesen werden',
  'harness.toast.pickFolder': 'Dieser Ordner lässt sich nicht öffnen',
  'harness.toast.openProject': 'Das Projekt lässt sich nicht öffnen',
  'harness.toast.closeProject': 'Das Projekt lässt sich nicht schließen',
  'harness.toast.forgetProject': 'Das Projekt konnte nicht vergessen werden',
};

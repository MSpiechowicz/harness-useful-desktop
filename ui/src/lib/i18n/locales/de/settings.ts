import type { settings as enSettings } from '../en/settings';
import type { AreaMessages } from '../types';

export const settings: AreaMessages<typeof enSettings> = {
  'settings.title': 'Einstellungen',
  'settings.context': 'Änderungen gelten sofort und werden auf diesem Gerät gespeichert',
  'settings.nav.label': 'Einstellungsbereiche',

  'settings.section.appearance': 'Darstellung',
  'settings.section.startup': 'Start',
  'settings.section.harnesses': 'Harnesses',
  'settings.section.projects': 'Projekte',
  'settings.section.updates': 'Updates',
  'settings.section.privacy': 'Datenschutz & Sicherheit',
  'settings.section.about': 'Info',

  'settings.keywords.appearance': 'design thema bewegung animation dunkel hell sprache',
  'settings.keywords.startup': 'intro start splash',
  'settings.keywords.harnesses': 'programmdatei binary omp pi pfad einrichtung',
  'settings.keywords.projects': 'zuletzt geöffnet ordner',
  'settings.keywords.updates': 'version update aktualisierung release installieren suchen neu',
  'settings.keywords.privacy': 'daten telemetrie diagnose löschen datenschutz',
  'settings.keywords.about': 'version build info',

  'settings.theme.title': 'Design',
  'settings.theme.description': 'Folge deinem System oder wähle eins. Die Änderung wird sofort als Vorschau gezeigt.',
  'settings.theme.system': 'System',
  'settings.theme.dark': 'Dunkel',
  'settings.theme.light': 'Hell',

  'settings.language.title': 'Sprache',
  'settings.language.description': 'Die Sprache der ganzen App. Die Änderung gilt sofort.',

  'settings.thinking.title': 'Denkprozess anzeigen',
  'settings.thinking.description': 'Die Überlegungen des Agenten, unter jeder Antwort eingeklappt. Lässt sich auch im Chat-Kopf umschalten.',
  'settings.aiNames.title': 'Chats mit KI benennen',
  'settings.aiNames.description': 'Ein kurzer Name und eine Beschreibung für jeden Chat, geschrieben vom Agenten des Chats direkt nach Ihrer ersten Nachricht.',
  'settings.aiNames.cost': 'Kostet eine sehr kleine Anfrage pro Chat, mit dem Modell des Chats.',
  'settings.aiNames.sent': 'Sendet nur die ersten 500 Zeichen dieser ersten Nachricht.',
  'settings.aiNames.off': 'Aus: Der Name sind die ersten Wörter, die Sie tippen, und nichts wird gesendet.',

  'settings.motion.title': 'Animationen',
  'settings.motion.description': 'Reduziert behält einfache Überblendungen; Aus schaltet alle Animationen ab. Die Einstellung deines Betriebssystems für weniger Bewegung gilt immer.',
  'settings.motion.full': 'Voll',
  'settings.motion.reduced': 'Reduziert',
  'settings.motion.off': 'Aus',

  'settings.intro.title': 'Intro beim Start abspielen',
  'settings.intro.description': 'Ein kurzer animierter Willkommensgruß beim Öffnen der App. Mit jeder Taste oder einem Klick überspringst du ihn.',
  'settings.sound.title': 'Intro-Sound',
  'settings.sound.description': 'Der Sound des Logos, während das Intro läuft. Er wird nie gespielt, wenn das Intro aus ist oder übersprungen wird.',
  'settings.replay.title': 'Intro erneut abspielen',
  'settings.replay.motionOff': 'Animationen sind aus; schalte sie ein, um das Intro erneut abzuspielen.',
  'settings.replay.ready': 'Noch einmal ansehen.',

  'settings.harness.binary': 'Programmdatei von {name}',
  'settings.harness.notFound': 'Auf diesem Rechner nicht gefunden',
  'settings.harness.choose': 'Auswählen…',
  'settings.harness.reset': 'Zurücksetzen',
  'settings.setup.title': 'Einrichtung',
  'settings.setup.description': 'Erkenne deine Harnesses erneut, wähle dein Standard-Harness und ein Projekt.',
  'settings.setup.run': 'Einrichtung erneut starten',

  'settings.recents.title': 'Zuletzt geöffnete Projekte merken',
  'settings.recents.description': 'Behalte eine Liste der Ordner, die du geöffnet hast. Beim Ausschalten wird die Liste geleert.',

  'settings.updates.version': 'Version {version}',
  'settings.updates.check': 'Jetzt suchen',
  'settings.updates.policy.title': 'Beim Start der App',
  'settings.updates.policy.label': 'Update-Verhalten',
  'settings.updates.policy.startupAsk': 'Erst fragen',
  'settings.updates.policy.autoInstall': 'Automatisch',
  'settings.updates.policy.off': 'Aus',
  'settings.updates.policy.startupAsk.hint': 'Sucht beim Start der App nach einer neuen Version und fragt, bevor sie installiert wird.',
  'settings.updates.policy.autoInstall.hint': 'Sucht beim Start der App und installiert eine neue Version. Du entscheidest, wann neu gestartet wird.',
  'settings.updates.policy.off.hint': 'Sucht nie von selbst. Nutze „Jetzt suchen“, wenn du möchtest.',
  'settings.updates.companions.title': 'Begleit-Apps auf Updates prüfen',
  'settings.updates.companions.description':
    'Wenn du Ecosystem öffnest, fragt die App GitHub nach den neuesten Releases von Harness Dashboard, Skills und Sidebar, höchstens alle sechs Stunden, solange die App läuft. GitHub sieht deine IP-Adresse und die App-Version, aber nicht, welche Begleit-Apps du hast.',
  'settings.updates.notConfigured': 'Dieser Build ist nicht für Updates eingerichtet und sucht deshalb nicht nach neuen Versionen. Es ist alles in Ordnung. Neue Versionen findest du auf der Releases-Seite.',
  'settings.updates.openReleases': 'Releases öffnen',
  'settings.updates.retry': 'Erneut versuchen',
  'settings.updates.notes': 'Versionshinweise',
  'settings.updates.install': 'Update installieren',
  'settings.updates.finish': 'Update abschließen',
  'settings.updates.restart': 'Neu starten und abschließen',

  'settings.about.label': 'Info',
  'settings.about.version': 'Version {version} · {os} {arch}',
  'settings.about.versionDebug': 'Version {version} · {os} {arch} · Debug-Build',
  'settings.about.localOnly': 'Nur lokal',
  'settings.about.links': 'Projekt-Links',
  'settings.about.source': 'Quellcode',
  'settings.about.releases': 'Releases',
  'settings.about.licence': 'Lizenz GPL-3.0-or-later',

  'settings.privacy.label': 'Was Harness Desktop mit Daten macht',
  'settings.privacy.stored.title': 'Auf diesem Gerät gespeichert',
  'settings.privacy.stored.text':
    'Nur App-Einstellungen: Design, Sprache, Animationen, Intro, Standard-Harness, Update-Verhalten, Fortschritt der Einrichtung, Größe und Position des Fensters, Diff-Layout, der zuletzt geöffnete Ordner, Pfade zuletzt geöffneter Projekte (falls gemerkt) und von dir gewählte Programmpfade. Sie liegen im App-Konfigurationsordner deines Betriebssystems. Es werden keine Chat-Inhalte, Tokens oder Passwörter gespeichert.',
  'settings.privacy.neverRead.title': 'Wird nie gelesen',
  'settings.privacy.neverRead.text':
    'Speicher für Zugangsdaten (auth.json, mcp-auth.json, credentials, .env-Dateien), private Schlüssel (.pem, .key, .p12, SSH-Schlüssel), Datenbanken (.db, .sqlite), Logdateien, Blob-Caches der Agenten und Terminal-Sitzungen. Die Prüfung folgt auch symbolischen Links.',
  'settings.privacy.redacted.title': 'Geschwärzt angezeigt',
  'settings.privacy.redacted.text':
    'Konfigurationswerte, die wie Geheimnisse aussehen, erscheinen verborgen: API-Schlüssel und Tokens, Umgebungsvariablen und Header von MCP-Servern, Auth- und OAuth-Felder, !command-Werte (werden nie ausgeführt) und Zugangsdaten in URLs. Umgebungsverweise wie $NAME zeigen nur den Namen. Fehlerausgaben von Agenten und Shell-Ausgaben werden geschwärzt, bevor sie auf den Bildschirm gelangen.',
  'settings.privacy.network.title': 'Netzwerk',
  'settings.privacy.network.text':
    'Die App selbst stellt zwei Arten von Anfragen, beide an GitHub, und beide lassen sich ausschalten. Die Update-Prüfung lädt die Release-Datei latest.json; das geschieht nur, wenn dieser Build einen Update-Schlüssel hat, und folgt deiner Update-Einstellung. Die Prüfung der Begleit-Apps fragt GitHub nach der neuesten Release-Nummer von Harness Dashboard, Skills und Sidebar, wenn du Ecosystem öffnest, höchstens alle sechs Stunden, solange die App läuft (eine fehlgeschlagene Prüfung wird nach einigen Minuten wiederholt). Sie fragt immer nach allen dreien und verrät so nicht, welche du hast; GitHub sieht nur deine IP-Adresse und die App-Version. Links öffnen sich nur in deinem Browser, wenn du sie anklickst, und nur für Projekt- und Harness-Seiten, es sei denn, du erlaubst unten jeden https-Link. Die Widgets von Harness Dashboard nutzen einen lokalen Prozess über stdio, ohne Netzwerk. Harness-CLIs sprechen mit ihren Anbietern wie in deinem Terminal, und Erweiterungsaktionen führen die CLI aus, die dabei Pakete herunterladen kann, aber erst nach deiner Bestätigung.',
  'settings.privacy.telemetry.title': 'Keine Telemetrie',
  'settings.privacy.telemetry.text': 'Keine Analysen, kein Tracking, keine Konten, keine Anmeldung.',
  'settings.privacy.agents.title': 'Agenten',
  'settings.privacy.agents.text':
    'Agenten laufen auf deinem Rechner mit deiner Umgebung und Konfiguration. Harness Desktop ändert nie die Einstellungen deines Harness; eine Freigabe-Entscheidung, die du für einen Chat triffst, gilt nur für diesen Chat, und der aktive Modus erscheint im Chat-Kopf, wenn das Harness einen hat. Shell-Befehle aus der Chat-Eingabe fragen immer nach einer Bestätigung. Diese App lädt nichts hoch.',
  'settings.privacy.git.title': 'Git',
  'settings.privacy.git.text':
    'Die Ansicht „Änderungen“ führt dein lokales Git nur lesend aus (Status, Diff, Branch-Liste), mit ausgeschalteten Pagern, externen Diff-Programmen und Dateisystem-Monitoren und mit deaktivierten Abfragen von Zugangsdaten.',
  'settings.privacy.logs.title': 'Logs',
  'settings.privacy.logs.text':
    'Eine lokale Logdatei im Log-Ordner deines Betriebssystems hält fest, was die App tut, nie, was du sagst. Sie enthält keine Prompts, Chat-Inhalte oder Dateiinhalte und wird bei 1 MiB rotiert.',

  'settings.privacy.openLink.title': 'Jeden Web-Link öffnen',
  'settings.privacy.openLink.description':
    'Links in Chats, etwa ein Pull Request, den der Agent geöffnet hat, öffnen sich in deinem Browser. Aus: Nur Projekt- und Harness-Seiten werden geöffnet. Immer nur https.',
  'settings.privacy.recents.title': 'Zuletzt geöffnete Projekte',
  'settings.privacy.recents.description': 'Vergiss jeden gemerkten Ordner. Steht dein geöffnetes Projekt auf der Liste, wird es ebenfalls geschlossen.',
  'settings.privacy.recents.clear': 'Zuletzt geöffnete Projekte löschen',
  'settings.privacy.recents.confirmTitle': 'Zuletzt geöffnete Projekte löschen?',
  'settings.privacy.recents.confirmDescription': 'Jeder gemerkte Ordner wird vergessen. Steht dein geöffnetes Projekt auf der Liste, wird es ebenfalls geschlossen. Deine Dateien bleiben unberührt.',
  'settings.privacy.recents.confirmLabel': 'Liste löschen',
  'settings.privacy.recents.cleared': 'Zuletzt geöffnete Projekte gelöscht',
  'settings.privacy.diagnostics.title': 'Diagnose',
  'settings.privacy.diagnostics.description': 'Kopiert App-Version, Betriebssystem und Harness-Versionen für einen Fehlerbericht. Keine Pfade, Projekte oder Inhalte.',
  'settings.privacy.diagnostics.copy': 'Diagnose kopieren',
  'settings.privacy.diagnostics.copied': 'Diagnose kopiert',
  'settings.privacy.diagnostics.copiedDescription': 'Nur App-Version, Betriebssystem und Harness-Versionen. Keine Pfade oder Inhalte.',
  'settings.privacy.diagnostics.failed': 'Kopieren nicht möglich',
  'settings.privacy.diagnostics.failedDescription': 'Die Zwischenablage ist nicht verfügbar.',
};

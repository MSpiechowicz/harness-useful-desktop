import type { subagents as enSubagents } from '../en/subagents';
import type { AreaMessages } from '../types';

export const subagents: AreaMessages<typeof enSubagents> = {
  'subagents.background': 'im Hintergrund',
  'subagents.stop': 'Stoppen',
  'subagents.stop.hint': 'Nur diesen Subagenten stoppen, der Chat läuft weiter',

  'subagents.tabs.label': 'Ansicht des Subagenten',
  'subagents.tab.activity': 'Aktivität',
  'subagents.tab.brief': 'Auftrag',
  'subagents.tab.output': 'Ausgabe',
  'subagents.tab.result': 'Ergebnis',

  'subagents.stat.tokens': 'Tokens',
  'subagents.stat.cost': 'Kosten',
  'subagents.stat.context': 'Kontext',
  'subagents.steps': 'Schritte',
  'subagents.steps.empty': 'Noch keine Schritte',
  'subagents.step.says': 'Sagt',
  'subagents.step.you': 'Du',
  'subagents.step.running': 'läuft',
  'subagents.step.failed': 'fehlgeschlagen',
  'subagents.step.viaOrchestrator': 'An den Orchestrator gesendet',

  'subagents.brief.from': 'Orchestrator',
  'subagents.brief.to': 'an {title}',
  'subagents.brief.copy': 'Auftrag kopieren',
  'subagents.brief.spawned': 'Gestartet vom Aufruf {tool}',
  'subagents.brief.spawnedAt': 'Gestartet vom Aufruf {tool} um {time}',
  'subagents.brief.jump': 'Dorthin springen',
  'subagents.brief.later': 'Später gesendet',

  'subagents.output.live': 'live',
  'subagents.output.finished': 'beendet',
  'subagents.output.copy': 'Ausgabe kopieren',
  'subagents.output.last': 'Letzte {shown} von {total} Zeilen',
  'subagents.output.all': 'Alle {total} Zeilen',
  'subagents.output.showAll': 'Alle anzeigen',
  'subagents.output.showLast': 'Letzte Zeilen anzeigen',
  'subagents.report.title': 'Abschlussbericht',
  'subagents.report.sub': 'an den Orchestrator zurückgegeben',
  'subagents.report.copy': 'Bericht kopieren',

  'subagents.message.placeholderDirect': 'Nachricht an {name}…',
  'subagents.message.placeholderOrchestrator': 'Den Orchestrator um eine Fortsetzung bitten…',
  'subagents.message.label': 'Nachricht an den Subagenten',
  'subagents.message.send': 'Senden',
};

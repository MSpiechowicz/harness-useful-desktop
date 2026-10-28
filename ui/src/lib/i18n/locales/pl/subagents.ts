import type { subagents as enSubagents } from '../en/subagents';
import type { AreaMessages } from '../types';

export const subagents: AreaMessages<typeof enSubagents> = {
  'subagents.background': 'w tle',
  'subagents.stop': 'Zatrzymaj',
  'subagents.stop.hint': 'Zatrzymuje tylko tego subagenta, czat działa dalej',

  'subagents.tabs.label': 'Widok subagenta',
  'subagents.tab.activity': 'Aktywność',
  'subagents.tab.brief': 'Zlecenie',
  'subagents.tab.output': 'Wyjście',
  'subagents.tab.result': 'Wynik',

  'subagents.stat.tokens': 'Tokeny',
  'subagents.stat.cost': 'Koszt',
  'subagents.stat.context': 'Kontekst',
  'subagents.steps': 'Kroki',
  'subagents.steps.empty': 'Jeszcze nie ma kroków',
  'subagents.step.says': 'Mówi',
  'subagents.step.you': 'Ty',
  'subagents.step.running': 'w toku',
  'subagents.step.failed': 'błąd',
  'subagents.step.viaOrchestrator': 'Wysłano do orkiestratora',

  'subagents.brief.from': 'Orkiestrator',
  'subagents.brief.to': 'do: {title}',
  'subagents.brief.copy': 'Kopiuj zlecenie',
  'subagents.brief.spawned': 'Uruchomione przez wywołanie {tool}',
  'subagents.brief.spawnedAt': 'Uruchomione przez wywołanie {tool} o {time}',
  'subagents.brief.jump': 'Przejdź do niego',
  'subagents.brief.later': 'Wysłane później',

  'subagents.output.live': 'na żywo',
  'subagents.output.finished': 'zakończone',
  'subagents.output.copy': 'Kopiuj wyjście',
  'subagents.output.last': 'Ostatnie {shown} z {total} wierszy',
  'subagents.output.all': 'Wszystkie wiersze: {total}',
  'subagents.output.showAll': 'Pokaż wszystko',
  'subagents.output.showLast': 'Pokaż ostatnie wiersze',
  'subagents.report.title': 'Raport końcowy',
  'subagents.report.sub': 'zwrócony do orkiestratora',
  'subagents.report.copy': 'Kopiuj raport',

  'subagents.message.placeholderDirect': 'Wiadomość do {name}…',
  'subagents.message.placeholderOrchestrator': 'Poproś orkiestratora o dalszy ciąg…',
  'subagents.message.label': 'Wiadomość do subagenta',
  'subagents.message.send': 'Wyślij',
};

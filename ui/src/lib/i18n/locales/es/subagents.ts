import type { subagents as enSubagents } from '../en/subagents';
import type { AreaMessages } from '../types';

export const subagents: AreaMessages<typeof enSubagents> = {
  'subagents.background': 'en segundo plano',
  'subagents.stop': 'Detener',
  'subagents.stop.hint': 'Detiene solo este subagente, el chat sigue en marcha',

  'subagents.tabs.label': 'Vista del subagente',
  'subagents.tab.activity': 'Actividad',
  'subagents.tab.brief': 'Encargo',
  'subagents.tab.output': 'Salida',
  'subagents.tab.result': 'Resultado',

  'subagents.stat.tokens': 'Tokens',
  'subagents.stat.cost': 'Coste',
  'subagents.stat.context': 'Contexto',
  'subagents.steps': 'Pasos',
  'subagents.steps.empty': 'Aún no hay pasos',
  'subagents.step.says': 'Dice',
  'subagents.step.you': 'Tú',
  'subagents.step.running': 'en curso',
  'subagents.step.failed': 'fallido',
  'subagents.step.viaOrchestrator': 'Enviado al orquestador',

  'subagents.brief.from': 'Orquestador',
  'subagents.brief.to': 'para {title}',
  'subagents.brief.copy': 'Copiar encargo',
  'subagents.brief.spawned': 'Lanzado por la llamada {tool}',
  'subagents.brief.spawnedAt': 'Lanzado por la llamada {tool} a las {time}',
  'subagents.brief.jump': 'Ir a ella',
  'subagents.brief.later': 'Enviado después',

  'subagents.output.live': 'en vivo',
  'subagents.output.finished': 'terminado',
  'subagents.output.copy': 'Copiar salida',
  'subagents.output.last': 'Últimas {shown} de {total} líneas',
  'subagents.output.all': 'Las {total} líneas',
  'subagents.output.showAll': 'Mostrar todo',
  'subagents.output.showLast': 'Mostrar las últimas líneas',
  'subagents.report.title': 'Informe final',
  'subagents.report.sub': 'devuelto al orquestador',
  'subagents.report.copy': 'Copiar informe',

  'subagents.message.placeholderDirect': 'Mensaje para {name}…',
  'subagents.message.placeholderOrchestrator': 'Pide al orquestador que continúe…',
  'subagents.message.label': 'Mensaje para el subagente',
  'subagents.message.send': 'Enviar',
};

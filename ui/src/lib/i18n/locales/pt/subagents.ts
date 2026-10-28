import type { subagents as enSubagents } from '../en/subagents';
import type { AreaMessages } from '../types';

export const subagents: AreaMessages<typeof enSubagents> = {
  'subagents.background': 'em segundo plano',
  'subagents.stop': 'Parar',
  'subagents.stop.hint': 'Para só este subagente, o chat continua em execução',

  'subagents.tabs.label': 'Visão do subagente',
  'subagents.tab.activity': 'Atividade',
  'subagents.tab.brief': 'Tarefa',
  'subagents.tab.output': 'Saída',
  'subagents.tab.result': 'Resultado',

  'subagents.stat.tokens': 'Tokens',
  'subagents.stat.cost': 'Custo',
  'subagents.stat.context': 'Contexto',
  'subagents.steps': 'Passos',
  'subagents.steps.empty': 'Ainda não há passos',
  'subagents.step.says': 'Diz',
  'subagents.step.you': 'Você',
  'subagents.step.running': 'em andamento',
  'subagents.step.failed': 'falhou',
  'subagents.step.viaOrchestrator': 'Enviado ao orquestrador',

  'subagents.brief.from': 'Orquestrador',
  'subagents.brief.to': 'para {title}',
  'subagents.brief.copy': 'Copiar tarefa',
  'subagents.brief.spawned': 'Iniciado pela chamada {tool}',
  'subagents.brief.spawnedAt': 'Iniciado pela chamada {tool} às {time}',
  'subagents.brief.jump': 'Ir até ela',
  'subagents.brief.later': 'Enviado depois',

  'subagents.output.live': 'ao vivo',
  'subagents.output.finished': 'concluído',
  'subagents.output.copy': 'Copiar saída',
  'subagents.output.last': 'Últimas {shown} de {total} linhas',
  'subagents.output.all': 'Todas as {total} linhas',
  'subagents.output.showAll': 'Mostrar tudo',
  'subagents.output.showLast': 'Mostrar as últimas linhas',
  'subagents.report.title': 'Relatório final',
  'subagents.report.sub': 'devolvido ao orquestrador',
  'subagents.report.copy': 'Copiar relatório',

  'subagents.message.placeholderDirect': 'Mensagem para {name}…',
  'subagents.message.placeholderOrchestrator': 'Peça ao orquestrador para continuar…',
  'subagents.message.label': 'Mensagem para o subagente',
  'subagents.message.send': 'Enviar',
};

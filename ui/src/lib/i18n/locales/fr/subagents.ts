import type { subagents as enSubagents } from '../en/subagents';
import type { AreaMessages } from '../types';

export const subagents: AreaMessages<typeof enSubagents> = {
  'subagents.background': 'en arrière-plan',
  'subagents.stop': 'Arrêter',
  'subagents.stop.hint': 'Arrête seulement ce sous-agent, la conversation continue',

  'subagents.tabs.label': 'Vue du sous-agent',
  'subagents.tab.activity': 'Activité',
  'subagents.tab.brief': 'Mission',
  'subagents.tab.output': 'Sortie',
  'subagents.tab.result': 'Résultat',

  'subagents.stat.tokens': 'Tokens',
  'subagents.stat.cost': 'Coût',
  'subagents.stat.context': 'Contexte',
  'subagents.steps': 'Étapes',
  'subagents.steps.empty': 'Aucune étape pour l’instant',
  'subagents.step.says': 'Dit',
  'subagents.step.you': 'Vous',
  'subagents.step.running': 'en cours',
  'subagents.step.failed': 'échoué',
  'subagents.step.viaOrchestrator': 'Envoyé à l’orchestrateur',

  'subagents.brief.from': 'Orchestrateur',
  'subagents.brief.to': 'pour {title}',
  'subagents.brief.copy': 'Copier la mission',
  'subagents.brief.spawned': 'Lancé par l’appel {tool}',
  'subagents.brief.spawnedAt': 'Lancé par l’appel {tool} à {time}',
  'subagents.brief.jump': 'Y aller',
  'subagents.brief.later': 'Envoyé ensuite',

  'subagents.output.live': 'en direct',
  'subagents.output.finished': 'terminé',
  'subagents.output.copy': 'Copier la sortie',
  'subagents.output.last': '{shown} dernières lignes sur {total}',
  'subagents.output.all': 'Les {total} lignes',
  'subagents.output.showAll': 'Tout afficher',
  'subagents.output.showLast': 'Afficher les dernières lignes',
  'subagents.report.title': 'Rapport final',
  'subagents.report.sub': 'renvoyé à l’orchestrateur',
  'subagents.report.copy': 'Copier le rapport',

  'subagents.message.placeholderDirect': 'Message pour {name}…',
  'subagents.message.placeholderOrchestrator': 'Demander à l’orchestrateur de poursuivre…',
  'subagents.message.label': 'Message pour le sous-agent',
  'subagents.message.send': 'Envoyer',
};

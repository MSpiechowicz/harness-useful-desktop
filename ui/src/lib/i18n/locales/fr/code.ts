import type { code as enCode } from '../en/code';
import type { AreaMessages } from '../types';

export const code: AreaMessages<typeof enCode> = {
  'code.onboarding.title': 'Harness Code',
  'code.onboarding.body': 'Installe le serveur MCP de Harness Code, pour que chaque agent puisse retrouver où le code est défini et qui l’utilise. La colonne Harness Code MCP de l’étape suivante montre quels harnesses l’ont. Il suit l’interrupteur Graphe du code des Réglages.',
  'code.onboarding.registers': 'Enregistre `harness-code mcp` comme serveur MCP dans chaque harness détecté, tant que le Graphe du code est activé dans les Réglages.',
  'code.onboarding.mcp.title': 'Harness Code MCP',
  'code.onboarding.mcp.body': 'Permet à vos agents de retrouver où le code est défini et qui l’utilise, en enregistrant Harness Code comme serveur MCP.',
  'code.onboarding.mcp.note': 'Seule l’entrée harness-code est ajoutée, elle lance le serveur installé. L’ajouter active le Graphe du code dans les Réglages. Un fichier que l’app ne peut pas modifier sans risque est laissé tel quel.',
  'code.onboarding.mcp.needsCode': 'Nécessite Harness Code. Incluez son installation ci-dessus ou installez-le d’abord vous-même.',
  'code.onboarding.effects': 'L’installeur de Code fait aussi',
  'code.onboarding.done': 'Code',
  'code.onboarding.doneMcp': 'Code MCP',
};

import type { chatRequest as enChatRequest } from '../en/chat-request';
import type { AreaMessages } from '../types';

export const chatRequest: AreaMessages<typeof enChatRequest> = {
  'chat.request.agentAsks': 'L’agent demande',
  'chat.request.questionOf': 'Question {number} sur {total}',
  'chat.request.back': 'Retour',
  'chat.request.next': 'Suivant',
  'chat.request.ownAnswer': 'Autre (saisissez la vôtre)',
  'chat.request.recommended': 'Recommandé',
  'chat.request.pickAny': 'Sélectionnez tout ce qui s’applique',
  'chat.request.pickFirst': 'Choisissez d’abord une réponse',
  'chat.request.closes': 'Se ferme tout seul à la fin du délai',
  'chat.request.closesSending': 'À la fin du délai, vos réponses actuelles sont envoyées',
  'chat.request.yes': 'Oui',
  'chat.request.no': 'Non',
  'chat.request.cancel': 'Annuler',
  'chat.request.submit': 'Envoyer',
};

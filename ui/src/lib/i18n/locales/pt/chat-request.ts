import type { chatRequest as enChatRequest } from '../en/chat-request';
import type { AreaMessages } from '../types';

export const chatRequest: AreaMessages<typeof enChatRequest> = {
  'chat.request.agentAsks': 'O agente está perguntando',
  'chat.request.questionOf': 'Pergunta {number} de {total}',
  'chat.request.back': 'Voltar',
  'chat.request.next': 'Avançar',
  'chat.request.ownAnswer': 'Outra (digite a sua)',
  'chat.request.recommended': 'Recomendada',
  'chat.request.pickAny': 'Selecione todas as que se aplicam',
  'chat.request.pickFirst': 'Escolha uma resposta primeiro',
  'chat.request.closes': 'Fecha sozinho quando o tempo acabar',
  'chat.request.closesSending': 'Quando o tempo acabar, suas respostas até agora serão enviadas',
  'chat.request.yes': 'Sim',
  'chat.request.no': 'Não',
  'chat.request.cancel': 'Cancelar',
  'chat.request.submit': 'Enviar',
};

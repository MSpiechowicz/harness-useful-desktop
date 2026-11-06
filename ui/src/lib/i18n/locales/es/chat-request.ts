import type { chatRequest as enChatRequest } from '../en/chat-request';
import type { AreaMessages } from '../types';

export const chatRequest: AreaMessages<typeof enChatRequest> = {
  'chat.request.agentAsks': 'El agente pregunta',
  'chat.request.questionOf': 'Pregunta {number} de {total}',
  'chat.request.back': 'Atrás',
  'chat.request.next': 'Siguiente',
  'chat.request.ownAnswer': 'Otra (escribe la tuya)',
  'chat.request.recommended': 'Recomendada',
  'chat.request.pickAny': 'Selecciona todas las que correspondan',
  'chat.request.pickFirst': 'Elige primero una respuesta',
  'chat.request.closes': 'Se cierra sola cuando se acaba el tiempo',
  'chat.request.closesSending': 'Cuando se acabe el tiempo, se enviarán tus respuestas hasta ahora',
  'chat.request.yes': 'Sí',
  'chat.request.no': 'No',
  'chat.request.cancel': 'Cancelar',
  'chat.request.submit': 'Enviar',
};

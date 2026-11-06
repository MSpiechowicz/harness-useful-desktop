import type { chatRequest as enChatRequest } from '../en/chat-request';
import type { AreaMessages } from '../types';

export const chatRequest: AreaMessages<typeof enChatRequest> = {
  'chat.request.agentAsks': 'Der Agent fragt',
  'chat.request.questionOf': 'Frage {number} von {total}',
  'chat.request.back': 'Zurück',
  'chat.request.next': 'Weiter',
  'chat.request.ownAnswer': 'Andere (selbst eingeben)',
  'chat.request.recommended': 'Empfohlen',
  'chat.request.pickAny': 'Alles Zutreffende auswählen',
  'chat.request.pickFirst': 'Wähle zuerst eine Antwort',
  'chat.request.closes': 'Schließt sich von selbst, wenn die Zeit abläuft',
  'chat.request.closesSending': 'Wenn die Zeit abläuft, werden deine bisherigen Antworten gesendet',
  'chat.request.yes': 'Ja',
  'chat.request.no': 'Nein',
  'chat.request.cancel': 'Abbrechen',
  'chat.request.submit': 'Senden',
};

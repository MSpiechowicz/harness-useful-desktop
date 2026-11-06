import type { chatRequest as enChatRequest } from '../en/chat-request';
import type { AreaMessages } from '../types';

export const chatRequest: AreaMessages<typeof enChatRequest> = {
  'chat.request.agentAsks': 'Agent pyta',
  'chat.request.questionOf': 'Pytanie {number} z {total}',
  'chat.request.back': 'Wstecz',
  'chat.request.next': 'Dalej',
  'chat.request.ownAnswer': 'Inna (wpisz własną)',
  'chat.request.recommended': 'Polecane',
  'chat.request.pickAny': 'Zaznacz wszystkie pasujące',
  'chat.request.pickFirst': 'Najpierw wybierz odpowiedź',
  'chat.request.closes': 'Zamknie się sama po upływie czasu',
  'chat.request.closesSending': 'Po upływie czasu zostaną wysłane dotychczasowe odpowiedzi',
  'chat.request.yes': 'Tak',
  'chat.request.no': 'Nie',
  'chat.request.cancel': 'Anuluj',
  'chat.request.submit': 'Wyślij',
};

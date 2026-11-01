import type { code as enCode } from '../en/code';
import type { AreaMessages } from '../types';

export const code: AreaMessages<typeof enCode> = {
  'code.onboarding.title': 'Harness Code',
  'code.onboarding.body': 'Instaluje serwer MCP Harness Code, aby każdy agent mógł sprawdzać, gdzie kod jest zdefiniowany i kto go używa. Kolumna Harness Code MCP w następnym kroku pokazuje, które harnessy go mają. Podąża za przełącznikiem Graf kodu w Ustawieniach.',
  'code.onboarding.registers': 'Rejestruje `harness-code mcp` jako serwer MCP w każdym wykrytym harnessie, gdy Graf kodu jest włączony w Ustawieniach.',
  'code.onboarding.mcp.title': 'Harness Code MCP',
  'code.onboarding.mcp.body': 'Pozwala agentom sprawdzać, gdzie kod jest zdefiniowany i kto go używa, przez zarejestrowanie Harness Code jako serwera MCP.',
  'code.onboarding.mcp.note': 'Dodawany jest tylko wpis harness-code, który uruchamia zainstalowany serwer. Jego dodanie włącza Graf kodu w Ustawieniach. Plik, którego aplikacja nie może bezpiecznie edytować, zostaje nietknięty.',
  'code.onboarding.mcp.needsCode': 'Wymaga Harness Code. Zaznacz jego instalację powyżej albo zainstaluj go wcześniej samodzielnie.',
  'code.onboarding.effects': 'Instalator Code dodatkowo',
  'code.onboarding.done': 'Code',
  'code.onboarding.doneMcp': 'Code MCP',
};

import type { code as enCode } from '../en/code';
import type { AreaMessages } from '../types';

export const code: AreaMessages<typeof enCode> = {
  'code.onboarding.title': 'Harness Code',
  'code.onboarding.body': 'Installiert den Harness-Code-MCP-Server, damit jeder Agent nachschlagen kann, wo Code definiert ist und wer ihn verwendet. Die Spalte Harness Code MCP im nächsten Schritt zeigt, welche Harnesses ihn haben. Er folgt dem Schalter Code-Graph in den Einstellungen.',
  'code.onboarding.registers': 'Registriert `harness-code mcp` als MCP-Server in jedem erkannten Harness, solange der Code-Graph in den Einstellungen aktiv ist.',
  'code.onboarding.mcp.title': 'Harness Code MCP',
  'code.onboarding.mcp.body': 'Lässt deine Agenten nachschlagen, wo Code definiert ist und wer ihn verwendet, indem Harness Code als MCP-Server registriert wird.',
  'code.onboarding.mcp.note': 'Es wird nur der Eintrag harness-code hinzugefügt, der den installierten Server startet. Das Hinzufügen schaltet den Code-Graph in den Einstellungen ein. Eine Datei, die die App nicht sicher bearbeiten kann, bleibt unberührt.',
  'code.onboarding.mcp.needsCode': 'Braucht Harness Code. Wähle oben seine Installation aus oder installiere es vorher selbst.',
  'code.onboarding.effects': 'Der Code-Installer außerdem',
  'code.onboarding.done': 'Code',
  'code.onboarding.doneMcp': 'Code MCP',
};

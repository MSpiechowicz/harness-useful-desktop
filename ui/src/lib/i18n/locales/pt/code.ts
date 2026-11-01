import type { code as enCode } from '../en/code';
import type { AreaMessages } from '../types';

export const code: AreaMessages<typeof enCode> = {
  'code.onboarding.title': 'Harness Code',
  'code.onboarding.body': 'Instala o servidor MCP do Harness Code, para que todo agente possa encontrar onde o código é definido e quem o usa. A coluna MCP do Harness Code no próximo passo mostra quais harnesses o têm. Ele segue o interruptor Grafo de código nas Configurações.',
  'code.onboarding.registers': 'Registra `harness-code mcp` como servidor MCP em cada harness detectado, enquanto o Grafo de código está ativado nas Configurações.',
  'code.onboarding.mcp.title': 'MCP do Harness Code',
  'code.onboarding.mcp.body': 'Deixa seus agentes encontrarem onde o código é definido e quem o usa, registrando o Harness Code como servidor MCP.',
  'code.onboarding.mcp.note': 'Só a entrada harness-code é adicionada, e ela executa o servidor instalado. Adicioná-la ativa o Grafo de código nas Configurações. Um arquivo que o app não consegue editar com segurança é deixado como está.',
  'code.onboarding.mcp.needsCode': 'Precisa do Harness Code. Inclua a instalação dele acima ou instale-o você mesmo primeiro.',
  'code.onboarding.effects': 'O instalador do Code também',
  'code.onboarding.done': 'Code',
  'code.onboarding.doneMcp': 'MCP do Code',
};

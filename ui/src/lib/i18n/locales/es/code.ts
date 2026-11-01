import type { code as enCode } from '../en/code';
import type { AreaMessages } from '../types';

export const code: AreaMessages<typeof enCode> = {
  'code.onboarding.title': 'Harness Code',
  'code.onboarding.body': 'Instala el servidor MCP de Harness Code, para que cada agente pueda buscar dónde se define el código y quién lo usa. La columna Harness Code MCP del paso siguiente muestra qué harnesses lo tienen. Sigue el interruptor Grafo de código de Ajustes.',
  'code.onboarding.registers': 'Registra `harness-code mcp` como servidor MCP en cada harness detectado, mientras el Grafo de código está activado en Ajustes.',
  'code.onboarding.mcp.title': 'Harness Code MCP',
  'code.onboarding.mcp.body': 'Deja que tus agentes busquen dónde se define el código y quién lo usa, registrando Harness Code como servidor MCP.',
  'code.onboarding.mcp.note': 'Solo se añade la entrada harness-code, que ejecuta el servidor instalado. Al añadirla se activa el Grafo de código en Ajustes. Un archivo que la app no puede editar con seguridad se deja como está.',
  'code.onboarding.mcp.needsCode': 'Necesita Harness Code. Incluye su instalación arriba o instálalo antes tú mismo.',
  'code.onboarding.effects': 'El instalador de Code además',
  'code.onboarding.done': 'Code',
  'code.onboarding.doneMcp': 'Code MCP',
};

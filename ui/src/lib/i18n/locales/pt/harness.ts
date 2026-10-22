import type { harness as enHarness } from '../en/harness';
import type { AreaMessages } from '../types';

export const harness: AreaMessages<typeof enHarness> = {
  'harness.boundary.setup': 'A configuração',
  'harness.boundary.intro': 'A introdução',
  'harness.boundary.notifications': 'As notificações',

  'harness.gate.noneTitle': 'Nenhum harness encontrado',
  'harness.gate.noneBody': 'O Harness Desktop funciona com OMP, PI, OpenCode e Claude Code. Execute a configuração para detectá-los.',
  'harness.gate.missingTitle': '{name} não está instalado',
  'harness.gate.setup': 'Configurar {name}',
  'harness.gate.retry': 'Tentar de novo',
  'harness.gate.banner': '{name} não está instalado. Mostrando o que está no disco.',

  'harness.gate.config.missingBody': 'Instale-o, ou aponte o Harness Desktop para um binário existente, para ver a configuração dele aqui.',
  'harness.gate.config.errorTitle': 'Não foi possível ler a configuração',
  'harness.gate.config.loading': 'Carregando a configuração',
  'harness.gate.config.refreshing': 'Atualizando a configuração',
  'harness.gate.extensions.missingBody': 'Instale-o, ou aponte o Harness Desktop para um binário existente, para ver as extensões dele aqui.',
  'harness.gate.extensions.errorTitle': 'Não foi possível ler as extensões',
  'harness.gate.extensions.loading': 'Carregando as extensões',
  'harness.gate.extensions.refreshing': 'Atualizando as extensões',

  'harness.switcher.label': 'Trocar de harness',

  'harness.toast.pickBinary': 'Não foi possível usar esse arquivo',
  'harness.toast.clearBinary': 'Não foi possível redefinir o binário',
  'harness.toast.recents': 'Não foi possível ler os projetos recentes',
  'harness.toast.pickFolder': 'Não foi possível abrir essa pasta',
  'harness.toast.openProject': 'Não foi possível abrir o projeto',
  'harness.toast.closeProject': 'Não foi possível fechar o projeto',
  'harness.toast.forgetProject': 'Não foi possível esquecer o projeto',
};

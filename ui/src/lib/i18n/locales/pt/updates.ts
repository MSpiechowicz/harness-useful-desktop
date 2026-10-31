import type { updates as enUpdates } from '../en/updates';
import type { AreaMessages } from '../types';

export const updates: AreaMessages<typeof enUpdates> = {
  'updates.installerCloses': 'O instalador vai fechar o Harness Desktop e abri-lo de novo quando terminar.',
  'updates.restartStopsAgents': 'Os agentes em execução são interrompidos quando o app reinicia.',
  'updates.restartWarning.one': '{count} chat em execução será interrompido, e qualquer resposta em andamento será perdida.',
  'updates.restartWarning.other': '{count} chats em execução serão interrompidos, e qualquer resposta em andamento será perdida.',

  'updates.unit.kb': 'KB',
  'updates.unit.mb': 'MB',
  'updates.progress.of': '{downloaded} de {total}',

  'updates.failure.network.check': 'Não foi possível acessar o servidor de atualizações. Verifique sua conexão e tente de novo.',
  'updates.failure.network.install': 'O download foi interrompido. Nada foi alterado. Tente de novo.',
  'updates.failure.verification': 'Não foi possível verificar a atualização, e ela não foi instalada.',
  'updates.failure.manifest': 'As informações da atualização estavam incompletas ou não há nada para este sistema. Tente de novo mais tarde.',
  'updates.failure.installer': 'O instalador não terminou, ou o pedido de permissão foi cancelado. Nada foi alterado.',
  'updates.failure.busy': 'Uma atualização já está sendo instalada.',
  'updates.failure.stale': 'Essa atualização não está mais disponível. Verifique de novo.',
  'updates.failure.unknown': 'Algo deu errado durante a atualização. Nada foi alterado.',

  'updates.status.idle': 'Ainda não verificado nesta sessão.',
  'updates.status.checking': 'Verificando atualizações…',
  'updates.status.upToDate': 'Você está em dia. Verificado às {time}.',
  'updates.status.notConfigured': 'Esta versão não consegue verificar atualizações.',
  'updates.status.available': 'A versão {version} está disponível.',
  'updates.status.downloading': 'Baixando a versão {version}…',
  'updates.status.installing': 'Instalando a versão {version}…',
  'updates.status.readyToRelaunch': 'A versão {version} está instalada. Reinicie para concluir.',
  'updates.status.failedCheck': 'Não foi possível verificar atualizações.',
  'updates.status.failedInstall': 'A atualização não foi instalada.',

  'updates.install.restart': 'Instalar e reiniciar',

  'updates.notice.label': 'Atualização',
  'updates.notice.available': 'A versão {version} está disponível',
  'updates.notice.updating': 'Atualizando para a {version}',
  'updates.notice.installed': 'A versão {version} está instalada',
  'updates.notice.failed': 'A atualização não foi instalada',
  'updates.notice.availableBody': 'Reinicie depois de instalar para concluir',
  'updates.notice.restartBody': 'Reinicie para concluir. Os agentes em execução são interrompidos quando o app reinicia.',
  'updates.notice.install': 'Instalar',
  'updates.notice.view': 'Ver',
  'updates.notice.later': 'Depois',
  'updates.notice.finish': 'Concluir atualização',
  'updates.notice.restart': 'Reiniciar agora',
  'updates.notice.retry': 'Tentar de novo',
  'updates.notice.dismiss': 'Dispensar',

  'updates.progress.label': 'Progresso da atualização',
  'updates.progress.installing': 'Instalando…',
  'updates.progress.downloading': 'Baixando…',

  'updates.restart.title': 'Reiniciar agora?',
  'updates.restart.confirm': 'Reiniciar agora',
  'updates.restart.finish': 'Concluir atualização',
  'updates.restart.keepWorking': 'Continuar trabalhando',
  'updates.restart.failed': 'Não foi possível reiniciar',
};

import type { updates as enUpdates } from '../en/updates';
import type { AreaMessages } from '../types';

export const updates: AreaMessages<typeof enUpdates> = {
  'updates.installerCloses': 'El instalador cerrará Harness Desktop y lo volverá a abrir al terminar.',
  'updates.restartStopsAgents': 'Los agentes en ejecución se detienen al reiniciar la app.',
  'updates.restartWarning.one': 'Se detendrá {count} chat en ejecución, y se perderá cualquier respuesta en curso.',
  'updates.restartWarning.other': 'Se detendrán {count} chats en ejecución, y se perderá cualquier respuesta en curso.',

  'updates.unit.kb': 'KB',
  'updates.unit.mb': 'MB',
  'updates.progress.of': '{downloaded} de {total}',

  'updates.failure.network.check': 'No se pudo conectar con el servidor de actualizaciones. Comprueba tu conexión e inténtalo de nuevo.',
  'updates.failure.network.install': 'La descarga se interrumpió. No se cambió nada; inténtalo de nuevo.',
  'updates.failure.verification': 'No se pudo verificar la actualización y no se instaló.',
  'updates.failure.manifest': 'La información de la actualización estaba incompleta o no incluye nada para este sistema. Inténtalo más tarde.',
  'updates.failure.installer': 'El instalador no terminó, o se canceló la solicitud de permiso. No se cambió nada.',
  'updates.failure.busy': 'Ya se está instalando una actualización.',
  'updates.failure.stale': 'Esa actualización ya no está disponible. Vuelve a buscar.',
  'updates.failure.unknown': 'Algo salió mal al actualizar. No se cambió nada.',

  'updates.status.idle': 'Aún no se ha buscado en esta sesión.',
  'updates.status.checking': 'Buscando actualizaciones…',
  'updates.status.upToDate': 'Estás al día. Comprobado a las {time}.',
  'updates.status.notConfigured': 'Esta compilación no puede buscar actualizaciones.',
  'updates.status.available': 'La versión {version} está disponible.',
  'updates.status.downloading': 'Descargando la versión {version}…',
  'updates.status.installing': 'Instalando la versión {version}…',
  'updates.status.readyToRelaunch': 'La versión {version} está instalada. Reinicia para terminar.',
  'updates.status.failedCheck': 'No se pudo buscar actualizaciones.',
  'updates.status.failedInstall': 'La actualización no se instaló.',


  'updates.install.restart': 'Instalar y reiniciar',

  'updates.notice.label': 'Actualización',
  'updates.notice.available': 'La versión {version} está disponible',
  'updates.notice.updating': 'Actualizando a {version}',
  'updates.notice.installed': 'La versión {version} está instalada',
  'updates.notice.failed': 'La actualización no se instaló',
  'updates.notice.restartBody': 'Reinicia para terminar. Los agentes en ejecución se detienen al reiniciar la app.',
  'updates.notice.install': 'Instalar',
  'updates.notice.view': 'Ver',
  'updates.notice.later': 'Más tarde',
  'updates.notice.finish': 'Terminar la actualización',
  'updates.notice.restart': 'Reiniciar ahora',
  'updates.notice.retry': 'Reintentar',
  'updates.notice.dismiss': 'Descartar',

  'updates.progress.label': 'Progreso de la actualización',
  'updates.progress.installing': 'Instalando…',
  'updates.progress.downloading': 'Descargando…',

  'updates.restart.title': '¿Reiniciar ahora?',
  'updates.restart.confirm': 'Reiniciar ahora',
  'updates.restart.finish': 'Terminar la actualización',
  'updates.restart.keepWorking': 'Seguir trabajando',
  'updates.restart.failed': 'No se pudo reiniciar',
};

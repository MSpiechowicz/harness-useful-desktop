import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Configuración',
  'config.resize': 'Cambiar el tamaño de la lista de archivos',
  'config.empty': 'No se encontró ningún archivo de configuración para este harness.',

  'config.layer.user': 'Usuario',
  'config.layer.profile': 'Perfil',
  'config.layer.project': 'Proyecto',
  'config.format.text': 'Texto',

  'config.list.label': 'Archivos de configuración',
  'config.list.layer': 'Capa «{layer}»',
  'config.list.error': 'Error',
  'config.list.notFound': 'No encontrado',

  'config.detail.reveal': 'Mostrar en el explorador de archivos',
  'config.detail.readError': 'No se pudo leer este archivo',
  'config.detail.missingTitle': 'Aún no hay archivo',
  'config.detail.missingBody': 'El harness no tiene ajustes en esta ruta. Los ajustes de esta capa aparecerán cuando exista el archivo.',
  'config.detail.filter': 'Filtrar claves y valores',
  'config.detail.matches.one': '{count} coincidencia',
  'config.detail.matches.other': '{count} coincidencias',
  'config.detail.entries.one': '{count} entrada de primer nivel',
  'config.detail.entries.other': '{count} entradas de primer nivel',
  'config.detail.collapseAll': 'Contraer todo',
  'config.detail.expandAll': 'Expandir todo',
  'config.detail.treeLabel': 'Ajustes de {title}',
  'config.detail.secrets': 'Los secretos se ocultan aquí y nunca salen del backend.',
};

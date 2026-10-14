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
  'config.detail.treeLabel': 'Ajustes de {title}',
  'config.detail.secrets': 'Los secretos se ocultan aquí y nunca salen del backend.',
  'config.detail.aside': 'Detalles del archivo',

  'config.search.label': 'Buscar claves y valores',
  'config.search.placeholder': 'Buscar ajustes',
  'config.search.matches.one': '{count} coincidencia',
  'config.search.matches.other': '{count} coincidencias',

  'config.mode.label': 'Vista',
  'config.mode.structured': 'Estructurada',
  'config.mode.source': 'Código fuente',

  'config.menu.label': 'Más acciones',
  'config.menu.collapseAll': 'Contraer todo',
  'config.menu.expandAll': 'Expandir todo',
  'config.menu.copyPath': 'Copiar ruta',

  'config.overview.title': 'Resumen',
  'config.overview.entries': 'entradas de primer nivel',
  'config.overview.values': 'valores',
  'config.overview.depth': 'niveles de profundidad',
  'config.overview.secrets': 'secretos ocultos',

  'config.file.title': 'Archivo',
  'config.file.layer': 'Capa',
  'config.file.format': 'Formato',
  'config.file.profile': 'Perfil',
  'config.file.path': 'Ruta',

  'config.profiles.title': 'Perfiles',
  'config.profiles.hint': 'Inicia {harness} con --profile <name> para usar uno.',
  'config.profiles.less': 'Mostrar menos',
  'config.profiles.all': 'Mostrar las {count}',
};

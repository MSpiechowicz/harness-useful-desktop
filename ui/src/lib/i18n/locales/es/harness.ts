import type { harness as enHarness } from '../en/harness';
import type { AreaMessages } from '../types';

export const harness: AreaMessages<typeof enHarness> = {
  'harness.boundary.setup': 'La configuración inicial',
  'harness.boundary.intro': 'La introducción',
  'harness.boundary.notifications': 'Las notificaciones',

  'harness.gate.noneTitle': 'No se encontró ningún harness',
  'harness.gate.noneBody': 'Harness Desktop funciona con OMP, PI, OpenCode y Claude Code. Ejecuta la configuración para detectarlos.',
  'harness.gate.missingTitle': '{name} no está instalado',
  'harness.gate.setup': 'Configurar {name}',
  'harness.gate.retry': 'Reintentar',
  'harness.gate.banner': '{name} no está instalado. Se muestra lo que hay en el disco.',

  'harness.gate.config.missingBody': 'Instálalo, o indica a Harness Desktop un ejecutable existente, para ver aquí su configuración.',
  'harness.gate.config.errorTitle': 'No se pudo leer la configuración',
  'harness.gate.config.loading': 'Cargando la configuración',
  'harness.gate.config.refreshing': 'Actualizando la configuración',
  'harness.gate.extensions.missingBody': 'Instálalo, o indica a Harness Desktop un ejecutable existente, para ver aquí sus extensiones.',
  'harness.gate.extensions.errorTitle': 'No se pudieron leer las extensiones',
  'harness.gate.extensions.loading': 'Cargando las extensiones',
  'harness.gate.extensions.refreshing': 'Actualizando las extensiones',

  'harness.switcher.label': 'Cambiar de harness',

  'harness.toast.pickBinary': 'No se puede usar ese archivo',
  'harness.toast.clearBinary': 'No se pudo restablecer el ejecutable',
  'harness.toast.recents': 'No se pudieron leer los proyectos recientes',
  'harness.toast.pickFolder': 'No se puede abrir esa carpeta',
  'harness.toast.openProject': 'No se puede abrir el proyecto',
  'harness.toast.closeProject': 'No se puede cerrar el proyecto',
  'harness.toast.forgetProject': 'No se pudo olvidar el proyecto',
};

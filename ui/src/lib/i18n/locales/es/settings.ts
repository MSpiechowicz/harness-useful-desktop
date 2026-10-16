import type { settings as enSettings } from '../en/settings';
import type { AreaMessages } from '../types';

export const settings: AreaMessages<typeof enSettings> = {
  'settings.title': 'Ajustes',
  'settings.context': 'Los cambios se aplican al instante y se guardan en este dispositivo',
  'settings.nav.label': 'Secciones de ajustes',
  'settings.search.label': 'Buscar ajustes',
  'settings.search.placeholder': 'Buscar ajustes…',
  'settings.search.noMatch': 'Ningún ajuste coincide con la búsqueda.',

  'settings.section.appearance': 'Apariencia',
  'settings.section.startup': 'Inicio',
  'settings.section.harnesses': 'Harnesses',
  'settings.section.projects': 'Proyectos',
  'settings.section.updates': 'Actualizaciones',
  'settings.section.privacy': 'Privacidad y seguridad',
  'settings.section.sync': 'Compartir configuración',
  'settings.section.about': 'Acerca de',

  'settings.keywords.appearance': 'tema animaciones movimiento oscuro claro animación idioma',
  'settings.keywords.startup': 'intro inicio arranque presentación',
  'settings.keywords.harnesses': 'ejecutable binario omp pi ruta configuración memoria recuerdo harness-memory mcp repositorio recuperar notas chat nuevo',
  'settings.keywords.projects': 'recientes carpetas',
  'settings.keywords.updates': 'versión actualizar actualización release instalar buscar nueva',
  'settings.keywords.privacy': 'datos telemetría diagnóstico borrar privacidad',
  'settings.keywords.sync': 'exportar importar compartir copia código archivo extensiones plugins paquetes marketplaces mcp servidores equipo',
  'settings.keywords.about': 'versión compilación acerca',

  'settings.theme.title': 'Tema',
  'settings.theme.description': 'Sigue el de tu sistema o elige uno. El cambio se previsualiza al instante.',
  'settings.theme.system': 'Sistema',
  'settings.theme.dark': 'Oscuro',
  'settings.theme.light': 'Claro',

  'settings.language.title': 'Idioma',
  'settings.language.description': 'El idioma de toda la app. El cambio se aplica al instante.',

  'settings.thinking.title': 'Mostrar el proceso de razonamiento',
  'settings.thinking.description': 'El razonamiento del agente, plegado bajo cada respuesta. También se activa desde la cabecera del chat.',
  'settings.usage.title': 'Mostrar el coste bajo las respuestas',
  'settings.usage.description': 'El modelo, el número de tokens y el coste al final de cada respuesta. También se activa desde la cabecera del chat.',
  'settings.aiNames.title': 'Nombrar los chats con IA',
  'settings.aiNames.description': 'Un nombre y una descripción cortos para cada chat, escritos por el agente del propio chat justo después de tu primer mensaje.',
  'settings.aiNames.cost': 'Cuesta una petición muy pequeña por chat, con el modelo del chat.',
  'settings.aiNames.sent': 'Solo envía los primeros 500 caracteres de ese primer mensaje.',
  'settings.aiNames.off': 'Desactivado: el nombre son las primeras palabras que escribes y no se envía nada.',

  'settings.motion.title': 'Animaciones',
  'settings.motion.description': 'Reducidas conserva los fundidos simples; Sin animaciones las desactiva todas. El ajuste de movimiento reducido de tu sistema siempre se respeta.',
  'settings.motion.full': 'Completas',
  'settings.motion.reduced': 'Reducidas',
  'settings.motion.off': 'Sin animaciones',

  'settings.intro.title': 'Reproducir la intro al iniciar',
  'settings.intro.description': 'Una breve bienvenida animada al abrir la app. Cualquier tecla o clic la omite.',
  'settings.sound.title': 'Sonido de la intro',
  'settings.sound.description': 'El sonido del logotipo mientras se reproduce la intro. Nunca suena si la intro está desactivada u omitida.',
  'settings.replay.title': 'Repetir la intro',
  'settings.replay.motionOff': 'Las animaciones están desactivadas; actívalas para repetir la intro.',
  'settings.replay.ready': 'Velo una vez más.',

  'settings.harness.binary': 'Ejecutable de {name}',
  'settings.harness.notFound': 'No se encontró en este equipo',
  'settings.harness.choose': 'Elegir…',
  'settings.harness.reset': 'Restablecer',
  'settings.setup.title': 'Configuración inicial',
  'settings.setup.description': 'Vuelve a detectar tus harnesses, elige el que usas a diario y un proyecto.',
  'settings.setup.run': 'Repetir la configuración inicial',

  'settings.memory.title': 'Memoria del proyecto',
  'settings.memory.description': 'Da a tus harnesses una memoria local (harness-memory) de lo que se hizo, los problemas y sus soluciones, y dónde está cada cosa, por repositorio. Solo se guarda en este equipo. Al desactivarla, se quita de la configuración MCP de los harnesses.',

  'settings.memoryRecall.title': 'Añadir memoria a los chats nuevos',
  'settings.memoryRecall.description': 'En el primer mensaje de un chat nuevo se añaden para el agente las notas relevantes de la memoria del proyecto. Se envían a tu proveedor de modelos junto con el mensaje.',

  'settings.recents.title': 'Recordar proyectos recientes',
  'settings.recents.description': 'Guarda una lista de las carpetas que abriste. Al desactivarlo, se borra la lista.',

  'settings.updates.version': 'Versión {version}',
  'settings.updates.check': 'Buscar ahora',
  'settings.updates.policy.title': 'Al iniciar la app',
  'settings.updates.policy.label': 'Política de actualización',
  'settings.updates.policy.startupAsk': 'Preguntar antes',
  'settings.updates.policy.autoInstall': 'Automática',
  'settings.updates.policy.off': 'Desactivada',
  'settings.updates.policy.startupAsk.hint': 'Busca una versión nueva al iniciar la app y pregunta antes de instalarla.',
  'settings.updates.policy.autoInstall.hint': 'Busca al iniciar la app e instala la versión nueva. Tú eliges cuándo reiniciar.',
  'settings.updates.policy.off.hint': 'Nunca busca por sí sola. Usa Buscar ahora cuando quieras.',
  'settings.updates.companions.title': 'Buscar actualizaciones de la familia Harness',
  'settings.updates.companions.description':
    'Al abrir Ecosystem, pide a GitHub las últimas versiones de Harness Dashboard, Skills, Memory y Sidebar, como máximo cada seis horas mientras la app está en marcha. GitHub ve tu dirección IP y la versión de la app, no qué herramientas de la familia Harness tienes.',
  'settings.updates.extensions.title': 'Actualizar las extensiones automáticamente',
  'settings.updates.extensions.description':
    'Cada diez minutos la app actualiza los marketplaces e instala las versiones nuevas de los plugins y paquetes de tu harness, con un aviso cuando lo hace. Desactivado: actualiza a mano desde Extensiones.',
  'settings.updates.notConfigured': 'Esta compilación no está preparada para actualizaciones, así que no buscará versiones nuevas. No pasa nada. Descarga las versiones nuevas desde la página de versiones.',
  'settings.updates.openReleases': 'Abrir versiones',
  'settings.updates.retry': 'Reintentar',
  'settings.updates.notes': 'Notas de la versión',
  'settings.updates.finish': 'Terminar la actualización',
  'settings.updates.restart': 'Reiniciar para terminar',

  'settings.about.label': 'Acerca de',
  'settings.about.version': 'Versión {version} · {os} {arch}',
  'settings.about.versionDebug': 'Versión {version} · {os} {arch} · compilación de depuración',
  'settings.about.localOnly': 'Solo local',
  'settings.about.links': 'Enlaces del proyecto',
  'settings.about.source': 'Código fuente',
  'settings.about.releases': 'Versiones',
  'settings.about.licence': 'Licencia GPL-3.0-or-later',

  'settings.privacy.label': 'Qué hace Harness Desktop con los datos',
  'settings.privacy.stored.title': 'Guardado en este dispositivo',
  'settings.privacy.stored.text':
    'Solo las preferencias de la app: tema, idioma, animaciones, intro, harness predeterminado, política de actualización, progreso de la configuración, tamaño y posición de la ventana, disposición del diff, la última carpeta que abriste, las rutas de los proyectos recientes (si se recuerdan) y las rutas de ejecutables que elegiste. Están en la carpeta de configuración de apps de tu sistema. No se guarda contenido de chats, tokens ni contraseñas.',
  'settings.privacy.neverRead.title': 'Nunca se lee',
  'settings.privacy.neverRead.text':
    'Almacenes de credenciales (auth.json, mcp-auth.json, credentials, archivos .env), claves privadas (.pem, .key, .p12, claves SSH), bases de datos (.db, .sqlite), archivos de registro, cachés de blobs de los agentes y sesiones de terminal. La comprobación también sigue los enlaces simbólicos.',
  'settings.privacy.redacted.title': 'Se muestra censurado',
  'settings.privacy.redacted.text':
    'Los valores de configuración que parecen secretos aparecen ocultos: claves de API y tokens, variables de entorno y cabeceras de servidores MCP, campos de autenticación y OAuth, valores !command (nunca se ejecutan) y credenciales dentro de URL. Las referencias de entorno como $NAME muestran solo el nombre. La salida de errores de los agentes y la salida del shell se censuran antes de llegar a la pantalla.',
  'settings.privacy.network.title': 'Red',
  'settings.privacy.network.text':
    'La propia app hace dos tipos de solicitudes, ambas a GitHub, y ambas se pueden desactivar. La búsqueda de actualizaciones descarga el archivo de versión latest.json; solo ocurre si esta compilación tiene una clave de actualización y sigue tu ajuste de actualización. La comprobación de la familia Harness pide a GitHub el número de la última versión de Harness Dashboard, Skills, Memory y Sidebar al abrir Ecosystem, como máximo cada seis horas mientras la app está en marcha (si falla, se reintenta pasados unos minutos). Siempre pregunta por los cuatro, así que no revela cuáles tienes; GitHub solo ve tu dirección IP y la versión de la app. Los enlaces se abren en tu navegador solo cuando haces clic en ellos, y solo para sitios de proyectos y harnesses, salvo que permitas abajo todos los enlaces https. Los widgets de Harness Dashboard usan un proceso local mediante stdio, sin red. Las CLI de los harnesses hablan con sus proveedores como en tu terminal, y las acciones de las extensiones ejecutan la CLI, que puede descargar paquetes, solo después de que lo confirmes.',
  'settings.privacy.telemetry.title': 'Sin telemetría',
  'settings.privacy.telemetry.text': 'Sin analíticas, sin seguimiento, sin cuentas, sin inicio de sesión.',
  'settings.privacy.agents.title': 'Agentes',
  'settings.privacy.agents.text':
    'Los agentes se ejecutan en tu equipo con tu entorno y tu configuración. Harness Desktop nunca edita los ajustes de tu harness; la opción de aprobación que eliges para un chat se aplica solo a ese chat, y el modo activo aparece en la cabecera del chat cuando el harness lo tiene. Los comandos de shell desde la entrada del chat siempre piden confirmación. Esta app no sube nada.',
  'settings.privacy.git.title': 'Git',
  'settings.privacy.git.text':
    'La vista Cambios ejecuta tu Git local en modo de solo lectura (estado, diff, lista de ramas), con paginadores, programas de diff externos y monitores del sistema de archivos desactivados, y con las solicitudes de credenciales deshabilitadas.',
  'settings.privacy.logs.title': 'Registros',
  'settings.privacy.logs.text':
    'Un archivo de registro local en la carpeta de registros de tu sistema anota lo que hace la app, nunca lo que dices. No contiene prompts, contenido de chats ni contenido de archivos, y se rota al llegar a 1 MiB.',

  'settings.privacy.openLink.title': 'Abrir cualquier enlace web',
  'settings.privacy.openLink.description':
    'Los enlaces de los chats, como un pull request que abrió el agente, se abren en tu navegador. Desactivado: solo se abren sitios de proyectos y harnesses. Solo https, más http a este equipo (localhost).',
  'settings.privacy.recents.title': 'Proyectos recientes',
  'settings.privacy.recents.description': 'Olvida todas las carpetas recordadas. Si tu proyecto abierto está en la lista, también se cierra.',
  'settings.privacy.recents.clear': 'Borrar proyectos recientes',
  'settings.privacy.recents.confirmTitle': '¿Borrar los proyectos recientes?',
  'settings.privacy.recents.confirmDescription': 'Se olvida cada carpeta recordada. Si tu proyecto abierto está en la lista, también se cierra. Tus archivos no se tocan.',
  'settings.privacy.recents.confirmLabel': 'Borrar lista',
  'settings.privacy.recents.cleared': 'Proyectos recientes borrados',
  'settings.privacy.diagnostics.title': 'Diagnóstico',
  'settings.privacy.diagnostics.description': 'Copia la versión de la app, el sistema operativo y las versiones de los harnesses para un informe de error. Sin rutas, proyectos ni contenido.',
  'settings.privacy.diagnostics.copy': 'Copiar diagnóstico',
  'settings.privacy.diagnostics.copied': 'Diagnóstico copiado',
  'settings.privacy.diagnostics.copiedDescription': 'Solo la versión de la app, el sistema operativo y las versiones de los harnesses. Sin rutas ni contenido.',
  'settings.privacy.diagnostics.failed': 'No se pudo copiar',
  'settings.privacy.diagnostics.failedDescription': 'El portapapeles no está disponible.',
};

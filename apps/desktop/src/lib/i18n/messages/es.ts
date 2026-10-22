import type { Message } from "./types";
import type { MessageKey } from "./zh";

export const es: Record<MessageKey, Message> = {
  // Navegación
  "nav.transcribe": "Transcribir",
  "nav.history": "Historial",
  "nav.model": "Modelos",
  "nav.settings": "Ajustes",
  "nav.home": "Inicio",
  "nav.manage": "Gestionar",
  "nav.workspace": "Espacio de trabajo",
  "nav.mainLabel": "Navegación principal",
  "nav.expandTitle": "Mostrar barra lateral (⌘\\)",
  "nav.collapseTitle": "Ocultar barra lateral (⌘\\)",

  // Barra de herramientas
  "toolbar.ariaLabel": "Barra de herramientas de transcripción",
  "toolbar.toLight": "Cambiar a apariencia clara",
  "toolbar.toDark": "Cambiar a apariencia oscura",
  "toolbar.defaultModel": "Modelo predeterminado",

  // Comunes
  "common.system": "Sistema",
  "common.cancel": "Cancelar",
  "common.delete": "Eliminar",
  "common.copied": "Copiado",
  "common.open": "Abrir",
  "common.noModel": "Ningún modelo seleccionado",
  "common.notSelected": "Sin seleccionar",

  // Estados de la tarea
  "state.running": "Transcribiendo",
  "state.completed": "Completado",
  "state.cancelled": "Cancelado",
  "state.failed": "Error",

  // Vista de transcripción
  "home.subtitle": "Arrastra archivos de audio o vídeo en cualquier momento, o abre una de las transcripciones de abajo.",
  "home.import": "Importar archivos",
  "transcribe.untitled": "Medio sin nombre",
  "transcribe.introTitle": "Iniciar una transcripción",
  "transcribe.introBody":
    "Arrastra un archivo de audio o vídeo, o elige uno con el botón de abajo; las importaciones se ponen en cola automáticamente.",
  "transcribe.dropAria": "Arrastra un archivo de audio o vídeo, o pulsa Intro para elegir uno",
  "transcribe.dropTitle": "Arrastra un archivo de audio o vídeo",
  "transcribe.dropSub":
    "Admite formatos de audio y vídeo habituales; puedes añadir varios archivos a la vez",
  "transcribe.chooseFile": "Elegir archivo",
  "transcribe.copyAll": "Copiar texto completo",
  "transcribe.model": "Modelo",
  "transcribe.chunksBefore": "Transcrito",
  "transcribe.chunksAfter": { one: "segmento", other: "segmentos" },
  "transcribe.progressIdle": "Reconociendo",
  "transcribe.progressPercent": "Procesado {percent}%",
  "transcribe.partialNote":
    "Este resultado es parcial e incompleto; se te volverá a preguntar al exportar.",
  "transcribe.preparingExport": "Preparando la exportación…",
  "transcribe.speed": "{value}× en tiempo real",
  "transcribe.etaSeconds": "quedan unos {seconds} s",
  "transcribe.etaMinutes": "quedan unos {minutes} min",
  "transcribe.etaHours": "quedan unas {hours} h {minutes} min",

  // Historial
  "search.placeholder": "Buscar nombres o contenido",
  "search.clear": "Borrar",
  "search.results": "{count} resultados",
  "search.empty": "Sin registros coincidentes",
  "history.loading": "Cargando el historial…",
  "history.emptyTitle": "Aún no hay historial",
  "history.emptyBody": "Cuando termine una transcripción, su resultado aparecerá aquí.",
  "time.today": "Hoy",
  "time.yesterday": "Ayer",

  // Lector
  "reader.recognizing": "Reconociendo: el texto aparecerá por segmentos en un momento.",
  "reader.noText": "Aún no hay texto que mostrar.",
  "reader.searchPlaceholder": "Buscar en esta transcripción",
  "reader.matchCount": "{count} coincidencias",
  "reader.noMatch": "Sin coincidencias",
  "reader.viewLatest": "Ver lo más reciente",

  // Modelos
  "model.loadedHint": "Cargado en memoria",
  "model.unknownSize": "Tamaño desconocido",
  "model.import": "Importar modelo",
  "model.emptyTitle": "Aún no has importado modelos",
  "model.emptyBody":
    "Descarga tú mismo un paquete de Whisper, Qwen3-ASR, Paraformer o SenseVoice de sherpa-onnx y luego elige la carpeta para importarlo. La importación se copia al directorio de la app; tus descargas originales no se tocan.",
  "model.chooseFolder": "Elegir carpeta de modelos",
  "model.defaultTag": "Predeterminado",
  "model.installedNotLoaded": "Instalado, no cargado",
  "model.setDefault": "Establecer como predeterminado",
  "model.currentDefault": "Predeterminado actual",
  "model.footer": {
    one: "{count} modelo en total · {size}. Al eliminar solo se quita la copia interna; tus descargas originales no se ven afectadas.",
    other: "{count} modelos en total · {size}. Al eliminar solo se quita la copia interna; tus descargas originales no se ven afectadas.",
  },

  // Ajustes
  "settings.general": "General",
  "settings.transcription": "Transcripción",
  "settings.storage": "Almacenamiento",
  "settings.integration": "Integraciones",
  "settings.about": "Acerca de",
  "settings.themeName": "Apariencia",
  "settings.themeDesc": "Al seguir al sistema, el claro y el oscuro cambian con macOS",
  "settings.themeLight": "Claro",
  "settings.themeDark": "Oscuro",
  "settings.serviceName": "Servicio local",
  "settings.mcpName": "MCP (stdio)",
  "settings.languageName": "Idioma de la interfaz",
  "settings.languageDesc":
    "Idioma de la interfaz de la app; «Sistema» usa el idioma de macOS al iniciar",
  "settings.audioLangName": "Idioma",
  "settings.audioLangDesc":
    "Detectado automáticamente por el modelo; los resultados conservan su idioma original",
  "settings.audioLangValue": "Detección automática",
  "settings.defaultModelDesc": "Solo afecta a las tareas creadas a partir de ahora",
  "settings.manageModels": "Gestionar modelos",
  "settings.libraryDir": "Carpeta de modelos",
  "settings.historyDir": "Resultados del historial",
  "settings.storageNote":
    "Mover la carpeta de modelos requiere primero una comprobación de espacio y un plan de reversión, por lo que aún no está disponible; eliminar un modelo solo quita su copia interna.",
  "settings.jobsDir": "Registros de tareas",
  "settings.dataDir": "Datos de la app",
  "settings.retentionName": "Limpieza automática",
  "settings.retentionDesc":
    "Mueve a la Papelera los resultados e registros más antiguos al superar el umbral; la biblioteca de modelos y tus archivos originales nunca se limpian (desactivado por defecto)",
  "settings.retentionOff": "Desactivada",
  "settings.retentionAge": "Por antigüedad",
  "settings.retentionSize": "Por tamaño",
  "settings.retentionDays": "{days} días",
  "settings.chunkNote":
    "La decodificación y el troceado ocurren en memoria; esta versión no crea archivos temporales.",
  "settings.countModels": {
    one: "{count} modelo · {size}",
    other: "{count} modelos · {size}",
  },
  "settings.countHistory": {
    one: "{count} entrada · {size}",
    other: "{count} entradas · {size}",
  },
  "settings.serviceRunning": "En ejecución · pid {pid}",
  "settings.serviceStopped":
    "Sin ejecutar: se inicia automáticamente cuando lo llama la CLI o MCP",
  "settings.refresh": "Actualizar",
  "settings.mcpDesc":
    "9 herramientas: modelos, enviar, estado, resultado, cola, cancelar, reintentar, exportar",
  "settings.copyConfig": "Copiar configuración",
  "settings.copyExample": "Copiar ejemplo",
  "settings.cliDesc": "Comparte el mismo servicio y la misma cola que MCP",
  "settings.troubleIntro":
    "Solución de problemas: el servicio solo escucha en un socket Unix local (permisos 0600); el acceso a archivos requiere autorización al iniciar con",
  "settings.troubleOutro": "; los registros van a la terminal y no guardan contenido de audio.",
  "settings.aboutDesc": "Transcripción local de audio y vídeo · Versión preliminar {version}",
  "settings.aboutBuild": "Compilación de desarrollo sin firmar",
  "settings.mcpCommandPlaceholder": "<ruta al ejecutable voice-to-text-mcp>",
  "settings.mcpPathPlaceholder": "/ruta/a/tus/medios",
  "settings.cliMediaPlaceholder": "<archivo multimedia>",
  "settings.cliModelPlaceholder": "<id del modelo>",

  // Diálogos y errores
  "dialog.mediaFilter": "Audio o vídeo",
  "dialog.importModelTitle": "Elegir carpeta de modelos",
  "dialog.deleteModelTitle": "Eliminar modelo",
  "dialog.deleteModelMessage":
    "¿Eliminar la copia interna de «{name}»? Tu descarga original no se borra; la copia se mueve a la Papelera.",
  "error.copyFailed": "No se pudo copiar: {error}",
};

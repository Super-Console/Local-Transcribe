import type { Message } from "./types";
import type { MessageKey } from "./zh";

export const de: Record<MessageKey, Message> = {
  // Navigation
  "nav.transcribe": "Transkribieren",
  "nav.history": "Verlauf",
  "nav.model": "Modelle",
  "nav.settings": "Einstellungen",
  "nav.home": "Start",
  "nav.manage": "Verwalten",
  "nav.record": "Aufnehmen",
  "nav.workspace": "Arbeitsbereich",
  "nav.mainLabel": "Hauptnavigation",
  "nav.expandTitle": "Seitenleiste einblenden (⌘\\)",
  "nav.collapseTitle": "Seitenleiste ausblenden (⌘\\)",

  // Werkzeugleiste
  "toolbar.ariaLabel": "Transkriptions-Werkzeugleiste",
  "toolbar.toLight": "Zum hellen Erscheinungsbild wechseln",
  "toolbar.toDark": "Zum dunklen Erscheinungsbild wechseln",
  "toolbar.defaultModel": "Standardmodell",

  // Gemeinsam
  "common.system": "System",
  "common.cancel": "Abbrechen",
  "common.delete": "Löschen",
  "common.copied": "Kopiert",
  "common.open": "Öffnen",
  "common.noModel": "Kein Modell ausgewählt",
  "common.notSelected": "Nicht ausgewählt",

  // Auftragszustände
  "state.running": "Wird transkribiert",
  "state.completed": "Abgeschlossen",
  "state.cancelled": "Abgebrochen",
  "state.failed": "Fehlgeschlagen",

  // Transkriptionsansicht
  "home.subtitle": "Zieh jederzeit Audio- oder Videodateien hinein oder öffne unten eine Transkription.",
  "home.import": "Dateien importieren",
  "transcribe.untitled": "Unbenanntes Medium",
  "transcribe.introTitle": "Transkription starten",
  "transcribe.introBody":
    "Zieh eine Audio- oder Videodatei hierher oder wähle eine mit der Schaltfläche unten; Importe werden automatisch in die Warteschlange gestellt.",
  "transcribe.dropAria":
    "Audio- oder Videodatei hierher ziehen oder mit Eingabe eine auswählen",
  "transcribe.dropTitle": "Audio- oder Videodatei hierher ziehen",
  "transcribe.dropSub":
    "Häufige Audio- und Videoformate werden unterstützt; du kannst mehrere Dateien gleichzeitig hinzufügen",
  "transcribe.chooseFile": "Datei auswählen",
  "transcribe.copyAll": "Vollständigen Text kopieren",
  "transcribe.model": "Modell",
  "transcribe.chunksBefore": "Transkribiert",
  "transcribe.chunksAfter": { one: "Segment", other: "Segmente" },
  "transcribe.progressIdle": "Wird erkannt",
  "transcribe.progressPercent": "{percent} % verarbeitet",
  "transcribe.partialNote":
    "Dieses Ergebnis ist unvollständig; beim Exportieren wird noch einmal nachgefragt.",
  "transcribe.preparingExport": "Export wird vorbereitet…",
  "transcribe.speed": "{value}× Echtzeit",
  "transcribe.etaSeconds": "noch ca. {seconds} s",
  "transcribe.etaMinutes": "noch ca. {minutes} min",
  "transcribe.etaHours": "noch ca. {hours} h {minutes} min",

  // Verlauf
  "search.placeholder": "Namen oder Transkript durchsuchen",
  "search.clear": "Löschen",
  "search.results": "{count} Treffer",
  "search.empty": "Keine passenden Einträge",
  "history.loading": "Verlauf wird geladen…",
  "history.emptyTitle": "Noch kein Verlauf",
  "history.emptyBody": "Sobald eine Transkription abgeschlossen ist, erscheint ihr Ergebnis hier.",
  "time.today": "Heute",
  "time.yesterday": "Gestern",

  // Leser
  "reader.recognizing": "Erkennung läuft — Text erscheint gleich segmentweise.",
  "reader.noText": "Noch kein Text zum Anzeigen.",
  "reader.searchPlaceholder": "In dieser Transkription suchen",
  "reader.matchCount": "{count} Treffer",
  "reader.noMatch": "Keine Treffer",
  "reader.viewLatest": "Neueste ansehen",

  // Modelle
  "record.subtitle": "Direkt in der App aufnehmen — Mikrofon und Meeting-App als zwei getrennte Spuren.",
  "record.micLabel": "Mikrofon",
  "record.appLabel": "Meeting-Audio",
  "record.appNone": "Nicht aufnehmen",
  "record.hint": "Jede Spur wird separat transkribiert und erzeugt einen eigenen Eintrag.",
  "record.start": "Aufnahme starten",
  "record.stop": "Stoppen und transkribieren",
  "record.idle": "Keine Aufnahme",
  "record.refresh": "Quellen aktualisieren",
  "record.permission": "Aufnehmen braucht Mikrofonzugriff; Meeting-Apps brauchen „Systemaudio aufnehmen“ (Systemeinstellungen → Datenschutz).",
  "record.empty": "Kein Audio aufgenommen. Prüfe Rechte und Quellen.",
  "model.loadedHint": "Im Speicher geladen",
  "model.unknownSize": "Größe unbekannt",
  "model.import": "Modell importieren",
  "model.emptyTitle": "Noch keine Modelle importiert",
  "model.emptyBody":
    "Lade dir selbst ein Whisper-, Qwen3-ASR-, Paraformer- oder SenseVoice-Exportpaket von sherpa-onnx herunter und wähle dann den Ordner zum Importieren. Der Import wird in den App-Ordner kopiert; deine Original-Downloads bleiben unberührt.",
  "model.chooseFolder": "Modellordner auswählen",
  "model.defaultTag": "Standard",
  "model.installedNotLoaded": "Installiert, nicht geladen",
  "model.setDefault": "Als Standard festlegen",
  "model.currentDefault": "Aktueller Standard",
  "model.footer": {
    one: "{count} Modell insgesamt · {size}. Beim Löschen wird nur die interne Kopie entfernt; deine Original-Downloads bleiben unberührt.",
    other: "{count} Modelle insgesamt · {size}. Beim Löschen wird nur die interne Kopie entfernt; deine Original-Downloads bleiben unberührt.",
  },

  // Einstellungen
  "settings.general": "Allgemein",
  "settings.transcription": "Transkription",
  "settings.storage": "Speicher",
  "settings.integration": "Integrationen",
  "settings.about": "Über",
  "settings.themeName": "Erscheinungsbild",
  "settings.themeDesc": "Im Systemmodus wechseln Hell und Dunkel mit macOS",
  "settings.themeLight": "Hell",
  "settings.themeDark": "Dunkel",
  "settings.serviceName": "Lokaler Dienst",
  "settings.mcpName": "MCP (stdio)",
  "settings.languageName": "Sprache der Oberfläche",
  "settings.languageDesc":
    "Sprache der App-Oberfläche; „System“ verwendet die macOS-Sprache beim Start",
  "settings.audioLangName": "Sprache",
  "settings.audioLangDesc":
    "Vom Modell automatisch erkannt; Ergebnisse behalten ihre Originalsprache",
  "settings.audioLangValue": "Automatische Erkennung",
  "settings.defaultModelDesc": "Betrifft nur neu erstellte Aufträge",
  "settings.manageModels": "Modelle verwalten",
  "settings.libraryDir": "Modellordner",
  "settings.historyDir": "Verlaufsergebnisse",
  "settings.storageNote":
    "Das Verschieben des Modellordners erfordert zuerst eine Speicherplatzprüfung und einen Rollback-Plan; daher ist es noch nicht freigegeben. Das Löschen entfernt nur die interne Kopie.",
  "settings.jobsDir": "Auftragsprotokolle",
  "settings.dataDir": "App-Daten",
  "settings.retentionName": "Automatisch aufräumen",
  "settings.retentionDesc":
    "Verschiebt die ältesten Verlaufsergebnisse und Auftragsprotokolle nach Überschreiten der Schwelle in den Papierkorb; Modellbibliothek und Originaldateien werden nie aufgeräumt (standardmäßig aus)",
  "settings.retentionOff": "Aus",
  "settings.retentionAge": "Nach Alter",
  "settings.retentionSize": "Nach Größe",
  "settings.retentionDays": "{days} Tage",
  "settings.chunkNote":
    "Dekodierung und Segmentierung laufen im Speicher; diese Version legt keine temporären Dateien an.",
  "settings.countModels": {
    one: "{count} Modell · {size}",
    other: "{count} Modelle · {size}",
  },
  "settings.countHistory": {
    one: "{count} Eintrag · {size}",
    other: "{count} Einträge · {size}",
  },
  "settings.serviceRunning": "Läuft · pid {pid}",
  "settings.serviceStopped":
    "Nicht aktiv: wird bei Aufruf durch CLI oder MCP automatisch gestartet",
  "settings.refresh": "Aktualisieren",
  "settings.mcpDesc":
    "9 Tools: Modelle, senden, Status, Ergebnis, Warteschlange, abbrechen, erneut versuchen, exportieren",
  "settings.copyConfig": "Konfiguration kopieren",
  "settings.copyExample": "Beispiel kopieren",
  "settings.cliDesc": "Nutzt denselben Dienst und dieselbe Warteschlange wie MCP",
  "settings.troubleIntro":
    "Fehlerbehebung: Der Dienst lauscht nur auf einem lokalen Unix-Socket (Rechte 0600); Dateizugriff muss beim Start mit",
  "settings.troubleOutro":
    " autorisiert werden; Logs erscheinen im Terminal und enthalten keine Audioinhalte.",
  "settings.aboutDesc": "Lokale Audio- und Video-Transkription · Vorschau {version}",
  "settings.aboutBuild": "Unsignierter Entwicklungsbau",
  "settings.mcpCommandPlaceholder": "<Pfad zur voice-to-text-mcp-Executable>",
  "settings.mcpPathPlaceholder": "/pfad/zu/deinen/medien",
  "settings.cliMediaPlaceholder": "<Mediendatei>",
  "settings.cliModelPlaceholder": "<Modell-id>",

  // Dialoge und Fehler
  "dialog.mediaFilter": "Audio oder Video",
  "dialog.importModelTitle": "Modellordner auswählen",
  "dialog.deleteModelTitle": "Modell löschen",
  "dialog.deleteModelMessage":
    "Die interne Kopie von „{name}“ löschen? Dein Original-Download bleibt erhalten; die Kopie wird in den Papierkorb gelegt.",
  "error.copyFailed": "Kopieren fehlgeschlagen: {error}",
};

//! Menu-bar / tray icon and its four-state status.
//!
//! Priority is fixed: computing > error > sharing > not sharing, matching the
//! addendum. A live pid with a failing health probe is reported as an error,
//! never as "sharing".

use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use voice_to_text_lan::client::LanClient;

use crate::lan_host::HostState;

const TRAY_ID: &str = "main";

// The tray crate renders the icon bitmap inside a fixed 18-point box, so the
// embed must be @2x density on macOS: a 22-pixel bitmap would be upscaled on a
// Retina menu bar and render soft next to the system icons.
#[cfg(target_os = "macos")]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-template@2x.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-windows.png");

static LOCALE: Mutex<String> = Mutex::new(String::new());

#[derive(Debug, Clone, PartialEq, Eq)]
enum Status {
    NotSharing,
    Sharing { address: String, port: u16 },
    Computing,
    Error,
}

struct Labels {
    open: &'static str,
    stop: &'static str,
    quit: &'static str,
    not_sharing: &'static str,
    computing: &'static str,
    error: &'static str,
    sharing: &'static str,
}

fn labels(locale: &str) -> Labels {
    match locale {
        "zh" => Labels {
            open: "打开主窗口",
            stop: "停止共享",
            quit: "退出",
            not_sharing: "未共享",
            computing: "正在计算…",
            error: "节点异常",
            sharing: "正在共享 {address}:{port}",
        },
        "de" => Labels {
            open: "Hauptfenster öffnen",
            stop: "Freigabe beenden",
            quit: "Beenden",
            not_sharing: "Keine Freigabe",
            computing: "Rechnet…",
            error: "Knotenfehler",
            sharing: "Freigabe {address}:{port}",
        },
        "es" => Labels {
            open: "Abrir ventana",
            stop: "Detener uso compartido",
            quit: "Salir",
            not_sharing: "Sin compartir",
            computing: "Calculando…",
            error: "Error del nodo",
            sharing: "Compartiendo {address}:{port}",
        },
        "fr" => Labels {
            open: "Ouvrir la fenêtre",
            stop: "Arrêter le partage",
            quit: "Quitter",
            not_sharing: "Partage inactif",
            computing: "Calcul…",
            error: "Erreur du nœud",
            sharing: "Partage {address}:{port}",
        },
        _ => Labels {
            open: "Open window",
            stop: "Stop sharing",
            quit: "Quit",
            not_sharing: "Not sharing",
            computing: "Computing…",
            error: "Node error",
            sharing: "Sharing {address}:{port}",
        },
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    // Embed resources so startup does not depend on the working directory.
    #[cfg(target_os = "macos")]
    let resource = "icons/tray-template@2x.png";
    #[cfg(not(target_os = "macos"))]
    let resource = "icons/tray-windows.png";
    let icon = tauri::image::Image::from_bytes(TRAY_ICON_BYTES).map_err(|error| {
        tauri::Error::AssetNotFound(format!("Unable to decode tray icon {resource}: {error}"))
    })?;
    let menu = build_menu(app, &Status::NotSharing)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_open" => show_main(app),
            "tray_stop" => {
                if let Some(state) = app.try_state::<std::sync::Arc<HostState>>() {
                    let _ = state.stop();
                }
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = refresh_now(&app).await;
                });
            }
            "tray_quit" => crate::request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_always_on_top(false);
    }
}

fn build_menu(app: &AppHandle, status: &Status) -> tauri::Result<Menu<tauri::Wry>> {
    let locale = LOCALE.lock().map(|value| value.clone()).unwrap_or_default();
    let labels = labels(&locale);
    let status_text = match status {
        Status::NotSharing => labels.not_sharing.to_owned(),
        Status::Computing => labels.computing.to_owned(),
        Status::Error => labels.error.to_owned(),
        Status::Sharing { address, port } => labels
            .sharing
            .replace("{address}", address)
            .replace("{port}", &port.to_string()),
    };
    let open = MenuItem::with_id(app, "tray_open", labels.open, true, None::<&str>)?;
    let status_item = MenuItem::with_id(app, "tray_status", status_text, false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let stop = MenuItem::with_id(app, "tray_stop", labels.stop, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray_quit", labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &status_item, &separator, &stop, &quit])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    NotSharing,
    Sharing,
    Computing,
    Error,
}

/// Fixed priority: health failure is an error even if a task looks active.
fn classify_status(running: bool, healthy: bool, has_active_task: bool) -> StatusKind {
    if !running {
        StatusKind::NotSharing
    } else if !healthy {
        StatusKind::Error
    } else if has_active_task {
        StatusKind::Computing
    } else {
        StatusKind::Sharing
    }
}

async fn current_status(app: &AppHandle) -> Status {
    let Some(state) = app.try_state::<std::sync::Arc<HostState>>() else {
        return Status::NotSharing;
    };
    let Ok(status) = state.status() else {
        return Status::Error;
    };
    if !status.running {
        return Status::NotSharing;
    }
    let client = LanClient::new(status.address.clone(), status.port);
    let healthy = matches!(
        tokio::time::timeout(Duration::from_millis(900), client.health()).await,
        Ok(Ok(true))
    );
    let active = if healthy {
        match tokio::time::timeout(Duration::from_millis(900), client.tasks()).await {
            Ok(Ok(tasks)) => tasks.iter().any(|task| !task.state.is_terminal()),
            _ => return Status::Error,
        }
    } else {
        false
    };
    match classify_status(status.running, healthy, active) {
        StatusKind::NotSharing => Status::NotSharing,
        StatusKind::Error => Status::Error,
        StatusKind::Computing => Status::Computing,
        StatusKind::Sharing => Status::Sharing {
            address: status.address,
            port: status.port,
        },
    }
}

async fn refresh_now(app: &AppHandle) -> tauri::Result<()> {
    let status = current_status(app).await;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let menu = build_menu(app, &status)?;
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

/// Poll every two seconds; a killed child turns the menu into "node error"
/// within one cycle.
pub fn spawn_status_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let _ = refresh_now(&app).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

/// Native menus do not follow i18n changes, so the frontend calls this after
/// switching language and the menu is rebuilt immediately.
pub fn set_locale(app: &AppHandle, locale: &str) {
    if let Ok(mut current) = LOCALE.lock() {
        *current = locale.to_owned();
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = refresh_now(&app).await;
    });
}

#[cfg(test)]
mod tests {
    use super::{StatusKind, classify_status};

    #[test]
    fn tray_priority_puts_computing_above_sharing_and_health_failure_above_all() {
        assert_eq!(classify_status(false, false, false), StatusKind::NotSharing);
        assert_eq!(classify_status(true, false, false), StatusKind::Error);
        assert_eq!(classify_status(true, false, true), StatusKind::Error);
        assert_eq!(classify_status(true, true, true), StatusKind::Computing);
        assert_eq!(classify_status(true, true, false), StatusKind::Sharing);
    }

    /// The tray crate renders the bitmap inside a fixed 18-point box. On a 2x
    /// menu bar that box is 36 physical pixels, so an embedded 22-pixel bitmap
    /// gets upscaled ~1.6x and the icon renders soft next to the system icons.
    /// The macOS embed must therefore be the @2x (44x44) bitmap, which the
    /// system downsamples to 36 physical pixels instead.
    #[cfg(target_os = "macos")]
    #[test]
    fn tray_icon_bitmap_is_retina_density() {
        let icon = tauri::image::Image::from_bytes(super::TRAY_ICON_BYTES)
            .expect("embedded macOS tray icon decodes");
        assert_eq!((icon.width(), icon.height()), (44, 44));
    }
}

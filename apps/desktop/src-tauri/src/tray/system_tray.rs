use crate::state::app_state::AppState;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

const TRAY_SHOW_APP: &str = "tray_show_app";
const TRAY_START_SERVER: &str = "tray_start_server";
const TRAY_STOP_SERVER: &str = "tray_stop_server";
const TRAY_QUIT_APP: &str = "tray_quit_app";

pub fn create_system_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show_app = MenuItem::with_id(app, TRAY_SHOW_APP, "Show App", true, None::<&str>)?;
    let start_server =
        MenuItem::with_id(app, TRAY_START_SERVER, "Start Server", true, None::<&str>)?;
    let stop_server = MenuItem::with_id(app, TRAY_STOP_SERVER, "Stop Server", true, None::<&str>)?;
    let quit_app = MenuItem::with_id(app, TRAY_QUIT_APP, "Quit App", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_app, &start_server, &stop_server, &quit_app])?;

    let icon_bytes = include_bytes!("../../icons/tray-icon.png");

    let tray_icon =
        Image::from_bytes(icon_bytes).expect("Fallo al construir la imagen del tray icon en Tauri");

    TrayIconBuilder::new()
        .menu(&menu)
        .icon(tray_icon)
        .on_menu_event(|app: &AppHandle<R>, event: MenuEvent| {
            let app_handle = app.app_handle();
            match event.id().as_ref() {
                TRAY_SHOW_APP => {
                    show_main_window(&app_handle);
                }
                TRAY_START_SERVER => {
                    let app_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<AppState>();
                        let mut service = state.core_service.lock().await;
                        let _ = service.start(9090, false).await;
                    });
                }
                TRAY_STOP_SERVER => {
                    let app_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<AppState>();
                        let mut service = state.core_service.lock().await;
                        let _ = service.stop().await;
                    });
                }
                TRAY_QUIT_APP => {
                    let app_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<AppState>();
                        let mut service = state.core_service.lock().await;
                        let _ = service.stop().await;
                        app_handle.exit(0);
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray: &TrayIcon<R>, event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(&tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

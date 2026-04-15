// #![allow(dead_code, unused_variables, unused_imports)]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_log::{
    Target, TargetKind,
    log::info,
};
use tauri_plugin_notification::NotificationExt as _;
use tauri_plugin_window_state::StateFlags;

use crate::monitor::InspectorHandle;
use crate::window_control::{hide_main_window, show_main_window, toggle_main_window};

pub mod commands;
pub mod monitor;
pub mod utils;
pub mod window_control;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let plugin_log = tauri_plugin_log::Builder::new()
        .level(if cfg!(debug_assertions) {
            tauri_plugin_log::log::LevelFilter::Debug
        } else {
            tauri_plugin_log::log::LevelFilter::Info
        })
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Webview),
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .build();

    let plugin_window_state = tauri_plugin_window_state::Builder::new()
        .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
        .build();

    let tray_notified = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app);
        }))
        .plugin(plugin_log)
        .plugin(plugin_window_state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle();
                hide_main_window(app);

                if tray_notified
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    app.notification()
                        .builder()
                        .title("Portrait Monitor Fixer")
                        .body("App is still running in the system tray.")
                        .show()
                        .ok();
                }
            }
        })
        .setup(|app| {
            let mut insp = InspectorHandle::default();
            insp.start(app.app_handle().to_owned());

            let insp = Mutex::new(insp);
            app.manage(insp);

            let _window = app.get_webview_window("main").unwrap();
            #[cfg(debug_assertions)]
            {
                _window.open_devtools();
            }

            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        show_main_window(app);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        toggle_main_window(&app);
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(gen_handlers!())
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            #[allow(clippy::single_match)]
            match event {
                tauri::RunEvent::ExitRequested { api, code, .. } => {
                    if code.is_none() {
                        api.prevent_exit();
                    } else {
                        info!("exit code: {:?}", code);
                    }
                }
                _ => {}
            }
        });
}

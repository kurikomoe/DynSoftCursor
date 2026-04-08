#[macro_export]
macro_rules! gen_handlers {
    () => {
        tauri::generate_handler![
            $crate::commands::greet,
            $crate::monitor::start_inspector,
            $crate::monitor::stop_inspector,
            $crate::monitor::toggle_mouse_mode,
            $crate::monitor::get_inspector_state,
        ]
    };
}

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

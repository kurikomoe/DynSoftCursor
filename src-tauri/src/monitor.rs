use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use tauri::{Emitter, State};

use crate::utils::{
    get_all_monitors, get_mouse_monitor, hardware_mouse, software_mouse, MonitorInfo,
    MonitorInfoDto, Orientation,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorMode {
    Software,
    Hardware,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub struct InspectorState {
    pub running: bool,
    pub current_monitor: MonitorInfoDto,
    pub cursor_mode: CursorMode,
}

pub struct InspectorHandle {
    pub state: Arc<Mutex<InspectorState>>,
    pub thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl Default for InspectorHandle {
    fn default() -> Self {
        hardware_mouse().ok();
        let monitors = get_all_monitors();
        let monitor = get_mouse_monitor(&monitors).unwrap_or_else(|| monitors[0].clone());
        let state = InspectorState {
            running: false,
            current_monitor: monitor.into(),
            cursor_mode: CursorMode::Hardware,
        };
        let state = Arc::new(Mutex::new(state));
        Self {
            state,
            thread_handle: None,
        }
    }
}

type NotifyFn = Arc<dyn Fn(&InspectorState) + Send + Sync>;

impl InspectorHandle {
    fn start_with_notifier(&mut self, notify: NotifyFn) {
        if self.state.lock().unwrap().running {
            return;
        }
        self.state.lock().unwrap().running = true;

        let state = Arc::clone(&self.state);
        let thread = thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(10));

            if !state.lock().unwrap().running {
                break;
            }

            let monitors = get_all_monitors();
            if let Some(monitor) = get_mouse_monitor(&monitors) {
                if state.lock().unwrap().current_monitor.path == monitor.path.to_string_lossy() {
                    continue;
                }

                match monitor.orientation {
                    Orientation::Default => {
                        Self::toggle_mouse_mode(state.clone(), CursorMode::Hardware).ok()
                    }
                    _ => Self::toggle_mouse_mode(state.clone(), CursorMode::Software).ok(),
                };

                {
                    let mut lock = state.lock().unwrap();
                    lock.current_monitor = monitor.into();
                    let snapshot = lock.clone();
                    notify(&snapshot);
                }
            }
        });

        self.thread_handle = Some(thread);
    }

    pub fn start(&mut self, app_handle: tauri::AppHandle) {
        let notify: NotifyFn = Arc::new(move |s| {
            let _ = app_handle.emit("inspector-update", s);
        });
        if self.state.lock().unwrap().running {
            return;
        }
        self.start_with_notifier(notify);
    }

    pub fn toggle_mouse_mode(
        state: Arc<Mutex<InspectorState>>,
        mode: CursorMode,
    ) -> anyhow::Result<()> {
        use anyhow::Context;
        match mode {
            CursorMode::Hardware => {
                hardware_mouse().context("toggle to hardware mouse failed")?;
                state.lock().unwrap().cursor_mode = CursorMode::Hardware;
            }
            CursorMode::Software => {
                software_mouse().context("toggle to software mouse failed")?;
                state.lock().unwrap().cursor_mode = CursorMode::Software;
            }
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.state.lock().unwrap().running = false;
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }

    pub fn get_state(&self) -> InspectorState {
        self.state.lock().unwrap().clone()
    }
}

#[tauri::command]
pub fn get_inspector_state(inspector: State<Mutex<InspectorHandle>>) -> InspectorState {
    inspector.lock().unwrap().get_state()
}

#[tauri::command]
pub fn toggle_mouse_mode(
    inspector: State<Mutex<InspectorHandle>>,
    app_handle: tauri::AppHandle,
    mode: CursorMode,
) -> InspectorState {
    let insp = inspector.lock().unwrap();
    InspectorHandle::toggle_mouse_mode(insp.state.clone(), mode).ok();
    let state = insp.get_state();
    let _ = app_handle.emit("inspector-update", &state);
    state
}

#[tauri::command]
pub fn start_inspector(inspector: State<Mutex<InspectorHandle>>, app_handle: tauri::AppHandle) {
    inspector.lock().unwrap().start(app_handle);
}

#[tauri::command]
pub fn stop_inspector(inspector: State<Mutex<InspectorHandle>>) {
    inspector.lock().unwrap().stop();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_start_emits_updates() {
        let mut insp = InspectorHandle::default();

        let notify: NotifyFn = Arc::new(move |s| {
            dbg!(s);
        });

        insp.start_with_notifier(notify);
        thread::sleep(Duration::from_secs(10));
        insp.stop();
    }
}

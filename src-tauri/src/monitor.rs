use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use tauri::{Emitter, State};

use crate::utils::{
    MonitorInfoDto, Orientation, get_all_monitors, get_mouse_monitor, hardware_mouse,
    software_mouse,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorMode {
    Software,
    Hardware,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub struct InspectorState {
    pub running: bool,
    pub current_monitor: Option<MonitorInfoDto>,
    pub cursor_mode: CursorMode,
}

pub struct InspectorHandle {
    pub state: Arc<Mutex<InspectorState>>,
    pub thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl Default for InspectorHandle {
    fn default() -> Self {
        hardware_mouse().ok();
        let state = InspectorState {
            running: false,
            current_monitor: None,
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

        let thread = {
            let state = Arc::clone(&self.state);
            let mut monitors = get_all_monitors();
            let mut last_refresh = std::time::Instant::now();

            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(10));

                    if !state.lock().unwrap().running {
                        break;
                    }

                    if last_refresh.elapsed() > Duration::from_secs(1) {
                        monitors = get_all_monitors();
                        last_refresh = std::time::Instant::now();
                    }

                    let lock = state.lock().unwrap();
                    let current_monitor = lock.current_monitor.clone();
                    drop(lock);
                    if let Some(monitor) = get_mouse_monitor(&monitors) {
                        let monitor_dto: MonitorInfoDto = monitor.into();
                        if let Some(old_monitor) = current_monitor.as_ref()
                            && old_monitor.path == monitor_dto.path
                            && old_monitor.orientation == monitor_dto.orientation
                        {
                            continue;
                        }

                        match monitor_dto.orientation {
                            Orientation::Default => {
                                Self::toggle_mouse_mode(state.clone(), CursorMode::Hardware).ok()
                            }
                            _ => Self::toggle_mouse_mode(state.clone(), CursorMode::Software).ok(),
                        };

                        {
                            let mut lock = state.lock().unwrap();
                            lock.current_monitor = Some(monitor_dto);
                            let snapshot = lock.clone();
                            drop(lock);
                            notify(&snapshot);
                        }
                    } else {
                        let refreshed = get_all_monitors();
                        let mut lock = state.lock().unwrap();
                        monitors = refreshed;
                        lock.current_monitor = None;
                        let snapshot = lock.clone();
                        drop(lock);
                        notify(&snapshot);
                    }
                }
            })
        };

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

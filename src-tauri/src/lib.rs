mod desktop_layer;
mod input_forwarder;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use desktop_layer::{
    attach_diagnostics, attach_to_desktop_icon_layer, cleanup_desktop_layer_before_exit,
    debug_snapshot, detach_from_desktop_icon_layer, is_attached_to_desktop_icon_layer,
    AttachDiagnostics,
};
use input_forwarder::start_input_forwarder;
use tauri::{Emitter, Manager, Position, Size, State};

struct ModeState {
    attached: Arc<AtomicBool>,
    allow_exit: Arc<AtomicBool>,
    cleanup_done: Arc<AtomicBool>,
}

#[tauri::command]
fn switch_to_attached(
    window: tauri::WebviewWindow,
    state: State<'_, ModeState>,
) -> Result<AttachDiagnostics, String> {
    let _ = window.emit("debug-snapshot", debug_snapshot("switch_to_attached:before", &window));
    window
        .set_focusable(false)
        .map_err(|error| error.to_string())?;
    window.set_resizable(false).map_err(|error| error.to_string())?;
    window
        .set_skip_taskbar(true)
        .map_err(|error| error.to_string())?;
    attach_to_desktop_icon_layer(&window).map_err(|error| error.to_string())?;
    state.attached.store(true, Ordering::Relaxed);
    let diagnostics = attach_diagnostics(&window);
    let _ = window.emit("debug-snapshot", debug_snapshot("switch_to_attached:after", &window));
    Ok(diagnostics)
}

#[tauri::command]
fn switch_to_detached(
    window: tauri::WebviewWindow,
    state: State<'_, ModeState>,
) -> Result<(), String> {
    let _ = window.emit("debug-snapshot", debug_snapshot("switch_to_detached:before", &window));
    state.attached.store(false, Ordering::Relaxed);
    detach_from_desktop_icon_layer(&window).map_err(|error| error.to_string())?;
    window
        .set_focusable(true)
        .map_err(|error| error.to_string())?;
    window.set_resizable(true).map_err(|error| error.to_string())?;
    window
        .set_skip_taskbar(false)
        .map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    let _ = window.emit("debug-snapshot", debug_snapshot("switch_to_detached:after", &window));
    Ok(())
}

#[tauri::command]
fn prepare_close_app(window: tauri::WebviewWindow, state: State<'_, ModeState>) {
    state.attached.store(false, Ordering::Relaxed);
    let _ = window.emit("debug-snapshot", debug_snapshot("prepare_close_app:before", &window));
    let _ = detach_from_desktop_icon_layer(&window);
    let _ = window.set_focusable(true);
    let _ = window.set_resizable(true);
    let _ = window.set_skip_taskbar(false);
    let _ = window.show();
    let _ = window.emit("debug-snapshot", debug_snapshot("prepare_close_app:after", &window));
    let _ = window.emit("close-prepared", ());
}

#[tauri::command]
fn finish_close_app(app: tauri::AppHandle, window: tauri::WebviewWindow, state: State<'_, ModeState>) {
    state.attached.store(false, Ordering::Relaxed);
    state.allow_exit.store(true, Ordering::Relaxed);
    let _ = window.emit("debug-snapshot", debug_snapshot("finish_close_app:before", &window));
    let _ = window.set_focusable(true);
    if !state.cleanup_done.swap(true, Ordering::Relaxed) {
        let _ = cleanup_desktop_layer_before_exit(&window);
    }
    let _ = window.emit("debug-snapshot", debug_snapshot("finish_close_app:after", &window));
    let _ = window.hide();
    app.exit(0);
}

#[tauri::command]
fn get_attach_diagnostics(window: tauri::WebviewWindow) -> AttachDiagnostics {
    attach_diagnostics(&window)
}

#[tauri::command]
fn get_debug_snapshot(window: tauri::WebviewWindow, phase: String) -> AttachDiagnostics {
    debug_snapshot(&phase, &window)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let attached = Arc::new(AtomicBool::new(true));
    let allow_exit = Arc::new(AtomicBool::new(false));
    let cleanup_done = Arc::new(AtomicBool::new(false));
    let attached_for_setup = Arc::clone(&attached);

    tauri::Builder::default()
        .manage(ModeState {
            attached: Arc::clone(&attached),
            allow_exit: Arc::clone(&allow_exit),
            cleanup_done: Arc::clone(&cleanup_done),
        })
        .invoke_handler(tauri::generate_handler![
            switch_to_attached,
            switch_to_detached,
            prepare_close_app,
            finish_close_app,
            get_attach_diagnostics,
            get_debug_snapshot
        ])
        .setup(move |app| {
            let window = app
                .get_webview_window("widget")
                .ok_or("widget window was not created")?;

            window.set_position(Position::Physical(tauri::PhysicalPosition {
                x: 220,
                y: 120,
            }))?;
            window.set_size(Size::Physical(tauri::PhysicalSize {
                width: 900,
                height: 560,
            }))?;

            if let Err(error) = attach_to_desktop_icon_layer(&window) {
                eprintln!("initial desktop attach failed, starting detached: {error}");
                attached_for_setup.store(false, Ordering::Relaxed);
                let _ = detach_from_desktop_icon_layer(&window);
                let _ = window.set_focusable(true);
                let _ = window.set_resizable(true);
                let _ = window.set_skip_taskbar(false);
            } else {
                let _ = window.set_focusable(false);
                let _ = window.set_resizable(false);
                let _ = window.set_skip_taskbar(true);
            }

            window.show()?;

            start_input_forwarder(window.clone(), Arc::clone(&attached_for_setup));
            start_desktop_layer_guard(window, Arc::clone(&attached_for_setup));

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app_handle, event| {
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { .. },
                label,
                ..
            } = &event
            {
                if label == "widget" {
                    let state = app_handle.state::<ModeState>();
                    state.attached.store(false, Ordering::Relaxed);
                    state.allow_exit.store(true, Ordering::Relaxed);
                    if let Some(window) = app_handle.get_webview_window("widget") {
                        if !state.cleanup_done.swap(true, Ordering::Relaxed) {
                            let _ = cleanup_desktop_layer_before_exit(&window);
                        }
                        let _ = window.hide();
                    }
                }
            }

            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !allow_exit.load(Ordering::Relaxed) {
                    api.prevent_exit();
                } else if let Some(window) = app_handle.get_webview_window("widget") {
                    let state = app_handle.state::<ModeState>();
                    if !state.cleanup_done.swap(true, Ordering::Relaxed) {
                        let _ = cleanup_desktop_layer_before_exit(&window);
                    }
                    let _ = window.hide();
                }
            }
        });
}

fn start_desktop_layer_guard(window: tauri::WebviewWindow, attached: Arc<AtomicBool>) {
    thread::spawn(move || loop {
        if !attached.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
            continue;
        }

        let attached_and_visible = is_attached_to_desktop_icon_layer(&window).unwrap_or(false);
        if !attached_and_visible {
            if let Err(error) = attach_to_desktop_icon_layer(&window) {
                eprintln!("failed to restore desktop layer: {error}");
            }

            if let Err(error) = window.show() {
                eprintln!("failed to show widget window: {error}");
            }
        }

        thread::sleep(Duration::from_millis(1000));
    });
}

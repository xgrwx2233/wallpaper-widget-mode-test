use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use tauri::{Emitter, Runtime};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
        WindowsAndMessaging::{GetCursorPos, GetWindowRect},
    },
};

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopInputEvent {
    kind: DesktopInputKind,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum DesktopInputKind {
    Click,
}

pub fn start_input_forwarder<R: Runtime>(
    window: tauri::WebviewWindow<R>,
    attached_mode: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let hwnd = match window.hwnd() {
            Ok(hwnd) => HWND(hwnd.0),
            Err(_) => return,
        };

        let mut was_left_down = false;

        loop {
            if !attached_mode.load(Ordering::Relaxed) {
                was_left_down = false;
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            let mut rect = RECT::default();
            let mut cursor = POINT::default();
            let ok = unsafe {
                GetWindowRect(hwnd, &mut rect).is_ok() && GetCursorPos(&mut cursor).is_ok()
            };

            if !ok {
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let inside = cursor.x >= rect.left
                && cursor.x < rect.right
                && cursor.y >= rect.top
                && cursor.y < rect.bottom;
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let local = POINT {
                x: cursor.x - rect.left,
                y: cursor.y - rect.top,
            };
            let left_down = unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 };

            if inside && left_down && !was_left_down {
                emit_input(&window, DesktopInputKind::Click, local, width, height);
            }

            was_left_down = left_down;
            thread::sleep(Duration::from_millis(24));
        }
    });
}

fn emit_input<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    kind: DesktopInputKind,
    point: POINT,
    width: i32,
    height: i32,
) {
    let _ = window.emit(
        "desktop-input",
        DesktopInputEvent {
            kind,
            x: point.x,
            y: point.y,
            width,
            height,
        },
    );
}

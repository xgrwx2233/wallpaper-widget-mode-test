use std::{thread, time::Duration};

use serde::Serialize;
use tauri::Runtime;
use windows::{
    core::{s, BOOL},
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, FindWindowA, FindWindowExA, GetParent, GetWindowLongPtrW,
            IsWindowVisible, SendMessageTimeoutA, SetParent, SetWindowLongPtrW, SetWindowPos,
            ShowWindow, GWL_STYLE, HWND_BOTTOM, HWND_TOP, SMTO_NORMAL,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
            SWP_SHOWWINDOW, SW_SHOW, WS_CHILD, WS_POPUP,
        },
    },
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const WORKERW_SPAWN_MESSAGE: u32 = 0x052C;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachDiagnostics {
    pub progman_found: bool,
    pub standard_worker_found: bool,
    pub progman_worker_found: bool,
    pub worker_found: bool,
    pub attached: bool,
    pub visible: bool,
    pub parent_is_worker_w: bool,
    pub hwnd: isize,
    pub parent: isize,
    pub worker_w: isize,
    pub error: Option<String>,
}

fn find_wallpaper_worker_w() -> Result<HWND> {
    unsafe {
        let progman = FindWindowA(s!("Progman"), None)?;

        let _ = SendMessageTimeoutA(
            progman,
            WORKERW_SPAWN_MESSAGE,
            WPARAM(0xD),
            LPARAM(0x1),
            SMTO_NORMAL,
            1000,
            None,
        );

        thread::sleep(Duration::from_millis(80));

        let mut worker_w = HWND::default();
        EnumWindows(
            Some(enum_windows_find_desktop_host),
            LPARAM(&mut worker_w as *mut HWND as isize),
        )?;

        if worker_w.is_invalid() {
            worker_w =
                FindWindowExA(Some(progman), None, s!("WorkerW"), None).unwrap_or_default();
        }

        if worker_w.is_invalid() {
            Err("WorkerW not found".into())
        } else {
            Ok(worker_w)
        }
    }
}

fn find_worker_w_behind_shell_view() -> Result<Option<HWND>> {
    let mut worker_w = HWND::default();
    unsafe {
        EnumWindows(
            Some(enum_windows_find_desktop_host),
            LPARAM(&mut worker_w as *mut HWND as isize),
        )?;
    }
    Ok((!worker_w.is_invalid()).then_some(worker_w))
}

extern "system" fn enum_windows_find_desktop_host(window: HWND, state: LPARAM) -> BOOL {
    unsafe {
        let shell_view =
            FindWindowExA(Some(window), None, s!("SHELLDLL_DefView"), None).unwrap_or_default();

        if shell_view.is_invalid() {
            return BOOL(1);
        }

        let worker_w = FindWindowExA(None, Some(window), s!("WorkerW"), None).unwrap_or_default();
        if !worker_w.is_invalid() {
            *(state.0 as *mut HWND) = worker_w;
            return BOOL(0);
        }

        BOOL(1)
    }
}

pub fn attach_to_desktop_icon_layer<R: Runtime>(window: &tauri::WebviewWindow<R>) -> Result<()> {
    let hwnd = HWND(window.hwnd()?.0);
    let worker_w = find_wallpaper_worker_w()?;

    unsafe {
        set_child_window_style(hwnd);

        if GetParent(hwnd).ok() != Some(worker_w) {
            let _ = SetParent(hwnd, Some(worker_w));
        }

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOOWNERZORDER
                | SWP_FRAMECHANGED
                | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);

        if GetParent(hwnd).ok() != Some(worker_w) {
            return Err(format!("failed to parent window to WorkerW 0x{:X}", worker_w.0 as isize).into());
        }
    }

    Ok(())
}

pub fn detach_from_desktop_icon_layer<R: Runtime>(window: &tauri::WebviewWindow<R>) -> Result<()> {
    let hwnd = HWND(window.hwnd()?.0);

    unsafe {
        set_top_level_window_style(hwnd);
        let _ = SetParent(hwnd, None);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );
    }

    Ok(())
}

pub fn is_attached_to_desktop_icon_layer<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<bool> {
    let hwnd = HWND(window.hwnd()?.0);
    let worker_w = find_wallpaper_worker_w()?;

    unsafe { Ok(GetParent(hwnd).ok() == Some(worker_w) && IsWindowVisible(hwnd).as_bool()) }
}

pub fn attach_diagnostics<R: Runtime>(window: &tauri::WebviewWindow<R>) -> AttachDiagnostics {
    let mut diagnostics = AttachDiagnostics::default();

    unsafe {
        let progman = FindWindowA(s!("Progman"), None).unwrap_or_default();
        diagnostics.progman_found = !progman.is_invalid();
        diagnostics.standard_worker_found = find_worker_w_behind_shell_view()
            .ok()
            .flatten()
            .is_some();
        diagnostics.progman_worker_found =
            !FindWindowExA(Some(progman), None, s!("WorkerW"), None)
                .unwrap_or_default()
                .is_invalid();
    }

    let hwnd = match window.hwnd() {
        Ok(raw) => HWND(raw.0),
        Err(error) => {
            diagnostics.error = Some(error.to_string());
            return diagnostics;
        }
    };

    diagnostics.hwnd = hwnd.0 as isize;

    match find_wallpaper_worker_w() {
        Ok(worker_w) => unsafe {
            diagnostics.worker_found = true;
            diagnostics.worker_w = worker_w.0 as isize;
            let parent = GetParent(hwnd).unwrap_or_default();
            diagnostics.parent = parent.0 as isize;
            diagnostics.parent_is_worker_w = parent == worker_w;
            diagnostics.visible = IsWindowVisible(hwnd).as_bool();
            diagnostics.attached = diagnostics.parent_is_worker_w && diagnostics.visible;
        },
        Err(error) => {
            diagnostics.error = Some(error.to_string());
        }
    }

    diagnostics
}

unsafe fn set_child_window_style(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    let next_style = (style as u32 | WS_CHILD.0) & !WS_POPUP.0;
    SetWindowLongPtrW(hwnd, GWL_STYLE, next_style as isize);
}

unsafe fn set_top_level_window_style(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    let next_style = (style as u32 | WS_POPUP.0) & !WS_CHILD.0;
    SetWindowLongPtrW(hwnd, GWL_STYLE, next_style as isize);
}

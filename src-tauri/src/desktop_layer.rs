use std::{
    mem::transmute,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::Runtime;
use windows::{
    core::{s, w, BOOL},
    Win32::{
        Foundation::{HWND, LPARAM, POINT, RECT, WPARAM, MAX_PATH},
        Graphics::Dwm::DwmFlush,
        Graphics::Gdi::{
            GdiFlush, GetDC, InvalidateRect, MapWindowPoints, PaintDesktop, RedrawWindow,
            ReleaseDC, UpdateWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_ERASENOW, RDW_FRAME,
            RDW_INVALIDATE, RDW_UPDATENOW,
        },
        UI::{
            Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST},
            WindowsAndMessaging::{
                CallWindowProcW, DestroyWindow, EnumWindows, FindWindowA, FindWindowExA,
                GetClassNameW, GetClientRect, GetForegroundWindow, GetParent, GetWindowLongPtrW,
                GetWindowRect, GetWindowTextW, IsWindowVisible, SendMessageTimeoutA, SetParent,
                SendMessageW, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
                SystemParametersInfoW, DefWindowProcW, GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC,
                HWND_BOTTOM, HWND_TOP, MA_NOACTIVATE, SMTO_NORMAL,
                SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETDESKWALLPAPER, SPI_SETDESKWALLPAPER,
                SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE,
                SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, WS_BORDER,
                WS_CHILD, WS_CAPTION, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_DLGFRAME,
                WS_EX_APPWINDOW, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_LAYERED,
                WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_EX_WINDOWEDGE,
                WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WM_MOUSEACTIVATE, WM_NCACTIVATE,
                WM_NCCALCSIZE, WM_NCPAINT, WM_SETREDRAW, WM_STYLECHANGED, WM_STYLECHANGING,
                WM_WINDOWPOSCHANGED, WM_WINDOWPOSCHANGING, STYLESTRUCT,
            },
        },
    },
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const WORKERW_SPAWN_MESSAGE: u32 = 0x052C;
const DWMNCRP_DISABLED: i32 = 1;

static ORIGINAL_WNDPROC: OnceLock<isize> = OnceLock::new();
static NATIVE_FRAME_DEBUG: OnceLock<Mutex<NativeFrameDebug>> = OnceLock::new();
static NATIVE_ATTACHED_MODE: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct NativeFrameDebug {
    count: u64,
    last_message: String,
    last_result: String,
}

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
    pub candidate_count: usize,
    pub class_name: String,
    pub title: String,
    pub style_hex: String,
    pub ex_style_hex: String,
    pub window_rect: RectDiagnostics,
    pub client_rect: RectDiagnostics,
    pub foreground: isize,
    pub is_foreground: bool,
    pub native_hook_installed: bool,
    pub native_msg: String,
    pub native_count: u64,
    pub phase: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RectDiagnostics {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub width: i32,
    pub height: i32,
}

fn desktop_host_candidates() -> Result<Vec<HWND>> {
    unsafe {
        let progman = FindWindowA(s!("Progman"), None)?;
        let mut candidates = Vec::new();

        for (wparam, lparam) in [(0xD, 0x1), (0, 0)] {
            let _ = SendMessageTimeoutA(
                progman,
                WORKERW_SPAWN_MESSAGE,
                WPARAM(wparam),
                LPARAM(lparam),
                SMTO_NORMAL,
                1000,
                None,
            );
            thread::sleep(Duration::from_millis(80));
        }

        let mut sibling_worker_w = HWND::default();
        let _ = EnumWindows(
            Some(enum_windows_find_desktop_host),
            LPARAM(&mut sibling_worker_w as *mut HWND as isize),
        );
        push_unique(&mut candidates, sibling_worker_w);

        collect_progman_worker_ws(progman, &mut candidates);

        if candidates.is_empty() {
            Err("WorkerW desktop host candidate not found".into())
        } else {
            Ok(candidates)
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
    let candidates = desktop_host_candidates()?;
    let mut attempted = Vec::new();

    unsafe {
        NATIVE_ATTACHED_MODE.store(true, Ordering::Relaxed);
        install_native_frame_guard(hwnd);
        set_borderless_child_window_style(hwnd);
        disable_dwm_non_client_rendering(hwnd);

        for candidate in candidates {
            if candidate.is_invalid() {
                continue;
            }

            attempted.push(format!("0x{:X}", candidate.0 as isize));

            if GetParent(hwnd).ok() != Some(candidate) {
                let _ = SetParent(hwnd, Some(candidate));
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
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
            );

            if GetParent(hwnd).ok() == Some(candidate) && IsWindowVisible(hwnd).as_bool() {
                return Ok(());
            }
        }
    }

    Err(format!(
        "failed to parent window to any desktop host; attempted {}",
        attempted.join(", ")
    )
    .into())
}

pub fn detach_from_desktop_icon_layer<R: Runtime>(window: &tauri::WebviewWindow<R>) -> Result<()> {
    let hwnd = HWND(window.hwnd()?.0);

    unsafe {
        NATIVE_ATTACHED_MODE.store(false, Ordering::Relaxed);
        install_native_frame_guard(hwnd);
        set_borderless_top_level_window_style(hwnd);
        disable_dwm_non_client_rendering(hwnd);
        let _ = SetParent(hwnd, None);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
        );
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

pub fn cleanup_desktop_layer_before_exit<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<()> {
    let hwnd = HWND(window.hwnd()?.0);

    unsafe {
        NATIVE_ATTACHED_MODE.store(false, Ordering::Relaxed);
        let old_parent = GetParent(hwnd).unwrap_or_default();
        let rect = current_window_rect(hwnd).map(expand_desktop_rect_for_nonclient_cache);
        let dirty_rect = rect.and_then(|rect| map_desktop_rect_to_window(old_parent, rect));

        // Win10 can keep the final WorkerW child frame as a stale wallpaper
        // bitmap. Stop new paints, hide, flush DWM, then redraw the exact old
        // area after removing the parent relationship.
        let _ = SendMessageW(hwnd, WM_SETREDRAW, Some(WPARAM(0)), Some(LPARAM(0)));
        set_borderless_top_level_window_style(hwnd);
        disable_dwm_non_client_rendering(hwnd);
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
        );
        let _ = DwmFlush();

        refresh_desktop_shell(Some(old_parent), dirty_rect.as_ref());
        refresh_desktop_shell(Some(old_parent), None);
        let _ = DwmFlush();
        thread::sleep(Duration::from_millis(180));

        let _ = DestroyWindow(hwnd);
        let _ = GdiFlush();
        let _ = DwmFlush();
        thread::sleep(Duration::from_millis(120));
        paint_desktop_background(old_parent);
        refresh_desktop_shell(Some(old_parent), dirty_rect.as_ref());
        refresh_desktop_shell(Some(old_parent), None);
        kick_progman_worker_w();
        thread::sleep(Duration::from_millis(150));
        refresh_desktop_shell(Some(old_parent), None);
        refresh_current_wallpaper();
        let _ = GdiFlush();
        let _ = DwmFlush();
    }

    Ok(())
}

pub fn is_attached_to_desktop_icon_layer<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<bool> {
    let hwnd = HWND(window.hwnd()?.0);
    let candidates = desktop_host_candidates()?;

    unsafe {
        let parent = GetParent(hwnd).ok();
        Ok(parent.is_some_and(|value| candidates.contains(&value)) && IsWindowVisible(hwnd).as_bool())
    }
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

    unsafe {
        fill_window_snapshot(hwnd, &mut diagnostics);
    }

    diagnostics.native_hook_installed = ORIGINAL_WNDPROC.get().is_some();
    if let Some(debug) = NATIVE_FRAME_DEBUG.get().and_then(|state| state.lock().ok()) {
        diagnostics.native_msg = debug.last_message.clone();
        diagnostics.native_count = debug.count;
    } else {
        diagnostics.native_msg = if diagnostics.native_hook_installed {
            "installed".to_string()
        } else {
            "not-installed".to_string()
        };
    }

    match desktop_host_candidates() {
        Ok(candidates) => unsafe {
            diagnostics.worker_found = true;
            diagnostics.candidate_count = candidates.len();
            diagnostics.worker_w = candidates.first().map(|hwnd| hwnd.0 as isize).unwrap_or(0);
            let parent = GetParent(hwnd).unwrap_or_default();
            diagnostics.parent = parent.0 as isize;
            diagnostics.parent_is_worker_w = candidates.contains(&parent);
            diagnostics.visible = IsWindowVisible(hwnd).as_bool();
            diagnostics.attached = diagnostics.parent_is_worker_w && diagnostics.visible;
            fill_window_snapshot(hwnd, &mut diagnostics);
        },
        Err(error) => {
            diagnostics.error = Some(error.to_string());
        }
    }

    diagnostics
}

pub fn debug_snapshot<R: Runtime>(phase: &str, window: &tauri::WebviewWindow<R>) -> AttachDiagnostics {
    let mut diagnostics = attach_diagnostics(window);
    diagnostics.phase = phase.to_string();
    diagnostics
}

fn push_unique(candidates: &mut Vec<HWND>, hwnd: HWND) {
    if !hwnd.is_invalid() && !candidates.contains(&hwnd) {
        candidates.push(hwnd);
    }
}

unsafe fn collect_progman_worker_ws(progman: HWND, candidates: &mut Vec<HWND>) {
    let mut previous: Option<HWND> = None;

    loop {
        let next = FindWindowExA(Some(progman), previous, s!("WorkerW"), None).unwrap_or_default();
        if next.is_invalid() {
            break;
        }

        push_unique(candidates, next);
        previous = Some(next);
    }
}

unsafe fn refresh_desktop_shell(old_parent: Option<HWND>, dirty_rect: Option<&RECT>) {
    if let Some(parent) = old_parent {
        refresh_window_rect(parent, dirty_rect);
        refresh_window(parent);
    }

    let progman = FindWindowA(s!("Progman"), None).unwrap_or_default();
    refresh_window_rect(progman, None);
    refresh_window(progman);

    if let Ok(candidates) = desktop_host_candidates() {
        for hwnd in candidates {
            refresh_window_rect(hwnd, dirty_rect);
            refresh_window(hwnd);
        }
    }

    EnumWindows(Some(enum_windows_refresh_desktop_windows), LPARAM(0)).ok();
    SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
}

unsafe fn kick_progman_worker_w() {
    let progman = FindWindowA(s!("Progman"), None).unwrap_or_default();
    if progman.is_invalid() {
        return;
    }

    for (wparam, lparam) in [(0xD, 0x1), (0, 0)] {
        let _ = SendMessageTimeoutA(
            progman,
            WORKERW_SPAWN_MESSAGE,
            WPARAM(wparam),
            LPARAM(lparam),
            SMTO_NORMAL,
            1000,
            None,
        );
        thread::sleep(Duration::from_millis(80));
    }
}

unsafe fn paint_desktop_background(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }

    let hdc = GetDC(Some(hwnd));
    if hdc.is_invalid() {
        return;
    }

    let _ = PaintDesktop(hdc);
    let _ = ReleaseDC(Some(hwnd), hdc);
    let _ = GdiFlush();
}

unsafe fn current_window_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    GetWindowRect(hwnd, &mut rect).ok()?;
    Some(rect)
}

unsafe fn fill_window_snapshot(hwnd: HWND, diagnostics: &mut AttachDiagnostics) {
    diagnostics.parent = GetParent(hwnd).unwrap_or_default().0 as isize;
    diagnostics.visible = IsWindowVisible(hwnd).as_bool();
    diagnostics.style_hex = format!("0x{:08X}", GetWindowLongPtrW(hwnd, GWL_STYLE) as u32);
    diagnostics.ex_style_hex = format!("0x{:08X}", GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
    diagnostics.class_name = get_class_name(hwnd);
    diagnostics.title = get_window_title(hwnd);

    let foreground = GetForegroundWindow();
    diagnostics.foreground = foreground.0 as isize;
    diagnostics.is_foreground = foreground == hwnd;

    if let Some(rect) = current_window_rect(hwnd) {
        diagnostics.window_rect = rect_to_diagnostics(rect);
    }

    let mut client_rect = RECT::default();
    if GetClientRect(hwnd, &mut client_rect).is_ok() {
        diagnostics.client_rect = rect_to_diagnostics(client_rect);
    }
}

fn rect_to_diagnostics(rect: RECT) -> RectDiagnostics {
    RectDiagnostics {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    }
}

unsafe fn get_class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut buffer);
    utf16_slice_to_string(&buffer[..len.max(0) as usize])
}

unsafe fn get_window_title(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let len = GetWindowTextW(hwnd, &mut buffer);
    utf16_slice_to_string(&buffer[..len.max(0) as usize])
}

fn utf16_slice_to_string(slice: &[u16]) -> String {
    String::from_utf16_lossy(slice)
}

fn expand_desktop_rect_for_nonclient_cache(rect: RECT) -> RECT {
    RECT {
        left: rect.left - 8,
        top: rect.top - 64,
        right: rect.right + 8,
        bottom: rect.bottom + 8,
    }
}

unsafe fn map_desktop_rect_to_window(hwnd: HWND, rect: RECT) -> Option<RECT> {
    if hwnd.is_invalid() {
        return None;
    }

    let mut points = [
        POINT {
            x: rect.left,
            y: rect.top,
        },
        POINT {
            x: rect.right,
            y: rect.bottom,
        },
    ];

    MapWindowPoints(None, Some(hwnd), &mut points);

    Some(RECT {
        left: points[0].x,
        top: points[0].y,
        right: points[1].x,
        bottom: points[1].y,
    })
}

unsafe fn refresh_current_wallpaper() {
    let mut path = [0u16; MAX_PATH as usize];

    let has_path = SystemParametersInfoW(
        SPI_GETDESKWALLPAPER,
        MAX_PATH,
        Some(path.as_mut_ptr() as _),
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
    )
    .is_ok()
        && path.first().is_some_and(|value| *value != 0);

    if has_path {
        let _ = SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(path.as_mut_ptr() as _),
            SPIF_SENDCHANGE | SPIF_UPDATEINIFILE,
        );
    } else {
        let _ = SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            None,
            SPIF_SENDCHANGE | SPIF_UPDATEINIFILE,
        );
    }

    thread::sleep(Duration::from_millis(250));
}

unsafe fn refresh_window(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }

    let _ = InvalidateRect(Some(hwnd), None, true);
    let _ = RedrawWindow(
        Some(hwnd),
        None,
        None,
        RDW_INVALIDATE | RDW_ERASE | RDW_ERASENOW | RDW_UPDATENOW | RDW_FRAME | RDW_ALLCHILDREN,
    );
    let _ = UpdateWindow(hwnd);
}

unsafe fn refresh_window_rect(hwnd: HWND, rect: Option<&RECT>) {
    if hwnd.is_invalid() {
        return;
    }

    let raw_rect = rect.map(|rect| rect as *const RECT);
    let _ = InvalidateRect(Some(hwnd), raw_rect, true);
    let _ = RedrawWindow(
        Some(hwnd),
        raw_rect,
        None,
        RDW_INVALIDATE | RDW_ERASE | RDW_ERASENOW | RDW_UPDATENOW | RDW_FRAME | RDW_ALLCHILDREN,
    );
    let _ = UpdateWindow(hwnd);
}

extern "system" fn enum_windows_refresh_desktop_windows(window: HWND, _state: LPARAM) -> BOOL {
    unsafe {
        let shell_view =
            FindWindowExA(Some(window), None, s!("SHELLDLL_DefView"), None).unwrap_or_default();
        if !shell_view.is_invalid() {
            refresh_window(window);
            refresh_window(shell_view);
        }
    }

    BOOL(1)
}

unsafe fn set_borderless_child_window_style(hwnd: HWND) {
    let _ = SetWindowTextW(hwnd, w!(""));
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    let next_style = (style as u32 | WS_CHILD.0 | WS_CLIPSIBLINGS.0 | WS_CLIPCHILDREN.0)
        & !WS_POPUP.0
        & !WS_CAPTION.0
        & !WS_THICKFRAME.0
        & !WS_SYSMENU.0
        & !WS_MINIMIZEBOX.0
        & !WS_MAXIMIZEBOX.0
        & !WS_BORDER.0
        & !WS_DLGFRAME.0;
    SetWindowLongPtrW(hwnd, GWL_STYLE, next_style as isize);

    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    let mut next_ex_style = (ex_style as u32 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0)
        & !WS_EX_APPWINDOW.0
        & !WS_EX_WINDOWEDGE.0
        & !WS_EX_CLIENTEDGE.0
        & !WS_EX_DLGMODALFRAME.0;

    next_ex_style &= !WS_EX_LAYERED.0;
    next_ex_style &= !WS_EX_TRANSPARENT.0;

    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_ex_style as isize);
    force_frame_refresh(hwnd);
}

unsafe fn set_borderless_top_level_window_style(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    let next_style = (style as u32 | WS_POPUP.0)
        & !WS_CHILD.0
        & !WS_CAPTION.0
        & !WS_THICKFRAME.0
        & !WS_SYSMENU.0
        & !WS_MINIMIZEBOX.0
        & !WS_MAXIMIZEBOX.0
        & !WS_BORDER.0
        & !WS_DLGFRAME.0;
    SetWindowLongPtrW(hwnd, GWL_STYLE, next_style as isize);

    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    let next_ex_style = (ex_style as u32 | WS_EX_TOOLWINDOW.0)
        & !WS_EX_APPWINDOW.0
        & !WS_EX_WINDOWEDGE.0
        & !WS_EX_CLIENTEDGE.0
        & !WS_EX_DLGMODALFRAME.0
        & !WS_EX_LAYERED.0
        & !WS_EX_TRANSPARENT.0
        & !WS_EX_NOACTIVATE.0;
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next_ex_style as isize);
    force_frame_refresh(hwnd);
}

unsafe fn force_frame_refresh(hwnd: HWND) {
    let _ = SetWindowPos(
        hwnd,
        None,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
    );
}

unsafe fn disable_dwm_non_client_rendering(hwnd: HWND) {
    let policy = DWMNCRP_DISABLED;
    let _ = windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
        hwnd,
        windows::Win32::Graphics::Dwm::DWMWA_NCRENDERING_POLICY,
        &policy as *const _ as *const core::ffi::c_void,
        std::mem::size_of_val(&policy) as u32,
    );
}

unsafe fn install_native_frame_guard(hwnd: HWND) {
    if ORIGINAL_WNDPROC.get().is_some() {
        return;
    }

    let previous = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, native_frame_guard_wnd_proc as *const () as isize);
    let _ = ORIGINAL_WNDPROC.set(previous);
    let _ = NATIVE_FRAME_DEBUG.set(Mutex::new(NativeFrameDebug::default()));
}

unsafe extern "system" fn native_frame_guard_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    let phase = format!("msg=0x{msg:04X} wp=0x{:X} lp=0x{:X}", wparam.0, lparam.0 as usize);

    match msg {
        WM_NCCALCSIZE => {
            record_native_frame_debug(&phase, "return 0");
            return windows::Win32::Foundation::LRESULT(0);
        }
        WM_NCPAINT | WM_NCACTIVATE => {
            record_native_frame_debug(&phase, "suppress nonclient paint");
            return windows::Win32::Foundation::LRESULT(0);
        }
        WM_MOUSEACTIVATE => {
            if NATIVE_ATTACHED_MODE.load(Ordering::Relaxed) {
                record_native_frame_debug(&phase, "MA_NOACTIVATE");
                return windows::Win32::Foundation::LRESULT(MA_NOACTIVATE as isize);
            }
        }
        WM_STYLECHANGING => {
            if lparam.0 != 0 {
                let style = &mut *(lparam.0 as *mut STYLESTRUCT);
                style.styleNew &= !WS_CAPTION.0;
                style.styleNew &= !WS_THICKFRAME.0;
                style.styleNew &= !WS_SYSMENU.0;
                style.styleNew &= !WS_MINIMIZEBOX.0;
                style.styleNew &= !WS_MAXIMIZEBOX.0;
                record_native_frame_debug(&phase, &format!("styleNew=0x{:08X}", style.styleNew));
            }
        }
        WM_STYLECHANGED | WM_WINDOWPOSCHANGING | WM_WINDOWPOSCHANGED => {
            record_native_frame_debug(&phase, "changed");
        }
        _ => {}
    }

    call_original_wnd_proc(hwnd, msg, wparam, lparam)
}

unsafe fn call_original_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if let Some(previous) = ORIGINAL_WNDPROC.get().copied().filter(|value| *value != 0) {
        let previous_proc: windows::Win32::UI::WindowsAndMessaging::WNDPROC =
            transmute(previous);
        let result = CallWindowProcW(previous_proc, hwnd, msg, wparam, lparam);
        record_native_frame_debug(&format!("msg=0x{msg:04X}"), &format!("call original -> 0x{:X}", result.0 as isize));
        return result;
    }

    let result = DefWindowProcW(hwnd, msg, wparam, lparam);
    record_native_frame_debug(&format!("msg=0x{msg:04X}"), &format!("defproc -> 0x{:X}", result.0 as isize));
    result
}

fn record_native_frame_debug(message: &str, result: &str) {
    let debug = NATIVE_FRAME_DEBUG.get_or_init(|| Mutex::new(NativeFrameDebug::default()));
    if let Ok(mut state) = debug.lock() {
        state.count = state.count.saturating_add(1);
        state.last_message = message.to_string();
        state.last_result = result.to_string();
    }
}


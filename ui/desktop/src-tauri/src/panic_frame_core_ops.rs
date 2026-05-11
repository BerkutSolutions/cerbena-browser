use super::*;

pub(crate) fn hide_panic_frame_windows(app_handle: &AppHandle, profile_id: Uuid) {
    if let Some(window) = app_handle.get_webview_window(&panic_frame_border_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
    }
    if let Some(window) = app_handle.get_webview_window(&panic_frame_label_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
    }
    if let Some(window) = app_handle.get_webview_window(&panic_frame_controls_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
    }
    if let Some(window) = app_handle.get_webview_window(&panic_frame_menu_label(profile_id)) {
        hide_native_overlay_window(&window);
        let _ = window.hide();
    }
}

pub(crate) fn panic_frame_border_label(profile_id: Uuid) -> String {
    format!("panic-frame-border-{profile_id}")
}

pub(crate) fn panic_frame_label_label(profile_id: Uuid) -> String {
    format!("panic-frame-label-{profile_id}")
}

pub(crate) fn panic_frame_controls_label(profile_id: Uuid) -> String {
    format!("panic-frame-controls-{profile_id}")
}

pub(crate) fn panic_frame_menu_label(profile_id: Uuid) -> String {
    format!("panic-frame-menu-{profile_id}")
}

pub(crate) fn ensure_panic_frame_windows(app_handle: &AppHandle, profile_id: Uuid) -> Result<(), String> {
    ensure_panic_frame_window(
        app_handle,
        profile_id,
        "border",
        &panic_frame_border_label(profile_id),
        true,
    )?;
    ensure_panic_frame_window(
        app_handle,
        profile_id,
        "label",
        &panic_frame_label_label(profile_id),
        true,
    )?;
    ensure_panic_frame_window(
        app_handle,
        profile_id,
        "controls",
        &panic_frame_controls_label(profile_id),
        false,
    )?;
    ensure_panic_frame_window(
        app_handle,
        profile_id,
        "menu",
        &panic_frame_menu_label(profile_id),
        false,
    )?;
    Ok(())
}

pub(crate) fn ensure_panic_frame_window(
    app_handle: &AppHandle,
    profile_id: Uuid,
    mode: &str,
    label: &str,
    ignore_cursor_events: bool,
) -> Result<(), String> {
    if app_handle.get_webview_window(label).is_some() {
        return Ok(());
    }

    let script = format!(
        "window.__PANIC_FRAME_OVERLAY = true; window.__PANIC_FRAME_PROFILE_ID = '{}'; window.__PANIC_FRAME_MODE = '{}';",
        profile_id, mode
    );
    let window = WebviewWindowBuilder::new(app_handle, label, WebviewUrl::App("index.html".into()))
        .title("Cerbena Panic Frame")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .initialization_script(&script)
        .build()
        .map_err(|e| e.to_string())?;
    window
        .set_ignore_cursor_events(ignore_cursor_events)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn update_panic_frame_window(
    app_handle: &AppHandle,
    label: &str,
    bounds: WindowBounds,
    mode: &str,
) -> Result<(), String> {
    let Some(window) = app_handle.get_webview_window(label) else {
        return Ok(());
    };

    let (x, y, width, height) = match mode {
        "label" => {
            let width = (bounds.width * 0.34).clamp(LABEL_MIN_WIDTH, LABEL_MAX_WIDTH);
            (
                bounds.x + (bounds.width - width) / 2.0,
                bounds.y - LABEL_LIFT,
                width,
                LABEL_HEIGHT,
            )
        }
        "controls" => (
            bounds.x + bounds.width - CONTROL_RIGHT_GAP - CONTROL_SIZE,
            (bounds.y + CONTROL_TOP_OFFSET).max(bounds.work_top),
            CONTROL_SIZE,
            CONTROL_SIZE,
        ),
        "menu" => {
            let max_x = (bounds.work_right - MENU_WIDTH).max(bounds.work_left);
            let preferred_x =
                bounds.x + bounds.width - CONTROL_RIGHT_GAP - MENU_WIDTH + CONTROL_SIZE;
            let x = preferred_x.clamp(bounds.work_left, max_x);
            let max_y = (bounds.work_bottom - MENU_HEIGHT).max(bounds.work_top);
            let preferred_y = bounds.y + CONTROL_SIZE + MENU_OFFSET_Y;
            let y = preferred_y.clamp(bounds.work_top, max_y);
            (x, y, MENU_WIDTH, MENU_HEIGHT)
        }
        _ => (
            bounds.x - FRAME_SIDE_BLEED,
            bounds.y - FRAME_TOP_BLEED,
            bounds.width + FRAME_SIDE_BLEED * 2.0,
            bounds.height + FRAME_TOP_BLEED + FRAME_BOTTOM_BLEED,
        ),
    };
    if mode == "menu" && !window.is_visible().map_err(|e| e.to_string())? {
        return Ok(());
    }
    let rounded_x = x.round() as i32;
    let rounded_y = y.round() as i32;
    let rounded_width = width.max(24.0).round() as u32;
    let rounded_height = height.max(24.0).round() as u32;
    window
        .set_position(Position::Physical(PhysicalPosition::new(
            rounded_x, rounded_y,
        )))
        .map_err(|e| e.to_string())?;
    window
        .set_size(Size::Physical(PhysicalSize::new(
            rounded_width,
            rounded_height,
        )))
        .map_err(|e| e.to_string())?;
    if mode == "border" || !window.is_visible().map_err(|e| e.to_string())? {
        window.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn show_panic_frame_menu(app_handle: &AppHandle, profile_id: Uuid) -> Result<(), String> {
    ensure_panic_frame_window(
        app_handle,
        profile_id,
        "menu",
        &panic_frame_menu_label(profile_id),
        false,
    )?;
    let menu = app_handle
        .get_webview_window(&panic_frame_menu_label(profile_id))
        .ok_or_else(|| "panic frame menu window not found".to_string())?;
    let controls = app_handle
        .get_webview_window(&panic_frame_controls_label(profile_id))
        .ok_or_else(|| "panic frame controls window not found".to_string())?;
    let control_pos = controls.outer_position().map_err(|e| e.to_string())?;
    let control_size = controls.outer_size().map_err(|e| e.to_string())?;
    let x = control_pos.x + control_size.width as i32 - MENU_WIDTH as i32;
    let y = control_pos.y + control_size.height as i32 + MENU_OFFSET_Y as i32;
    menu.set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    menu.set_size(Size::Physical(PhysicalSize::new(
        MENU_WIDTH.round() as u32,
        MENU_HEIGHT.round() as u32,
    )))
    .map_err(|e| e.to_string())?;
    menu.show().map_err(|e| e.to_string())?;
    menu.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn hide_panic_frame_menu(app_handle: &AppHandle, profile_id: Uuid) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window(&panic_frame_menu_label(profile_id)) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn panic_frame_show_menu(
    app: AppHandle,
    request: PanicFrameMenuRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    show_panic_frame_menu(&app, request.profile_id)?;
    Ok(ok(correlation_id, true))
}

#[tauri::command]
pub fn panic_frame_hide_menu(
    app: AppHandle,
    request: PanicFrameMenuRequest,
    correlation_id: String,
) -> Result<UiEnvelope<bool>, String> {
    hide_panic_frame_menu(&app, request.profile_id)?;
    Ok(ok(correlation_id, true))
}

#[cfg(target_os = "windows")]
pub(crate) fn query_main_window_bounds(pid: u32) -> Option<WindowBounds> {
    let hwnd = find_main_window_for_pid(pid)?;
    unsafe {
        if is_window_visible(hwnd) == 0 {
            return None;
        }

        let rect = query_window_rect(hwnd)?;
        let monitor = monitor_from_window(hwnd, MONITOR_DEFAULTTONEAREST)?;
        let info = get_monitor_info(monitor)?;

        Some(WindowBounds {
            x: rect.left as f64,
            y: rect.top as f64,
            width: (rect.right - rect.left) as f64,
            height: (rect.bottom - rect.top) as f64,
            work_left: info.rc_work.left as f64,
            work_top: info.rc_work.top as f64,
            work_right: info.rc_work.right as f64,
            work_bottom: info.rc_work.bottom as f64,
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn query_main_window_bounds(_pid: u32) -> Option<WindowBounds> {
    None
}

#[cfg(target_os = "windows")]
type Hwnd = *mut c_void;
#[cfg(target_os = "windows")]
type Hmonitor = *mut c_void;

#[cfg(target_os = "windows")]
const MONITOR_DEFAULTTONEAREST: u32 = 2;
#[cfg(target_os = "windows")]
const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
#[cfg(target_os = "windows")]
const SWP_NOACTIVATE: u32 = 0x0010;
#[cfg(target_os = "windows")]
const SWP_HIDEWINDOW: u32 = 0x0080;
#[cfg(target_os = "windows")]
const SW_HIDE: i32 = 0;

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct MonitorInfo {
    cb_size: u32,
    rc_monitor: Rect,
    rc_work: Rect,
    dw_flags: u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct EnumWindowState {
    pid: u32,
    best_hwnd: Hwnd,
    best_area: i64,
}

#[cfg(target_os = "windows")]
unsafe extern "system" {
    pub(crate) fn EnumWindows(
        lp_enum_func: Option<unsafe extern "system" fn(Hwnd, isize) -> i32>,
        l_param: isize,
    ) -> i32;
    pub(crate) fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    pub(crate) fn GetWindowThreadProcessId(hwnd: Hwnd, process_id: *mut u32) -> u32;
    pub(crate) fn IsWindowVisible(hwnd: Hwnd) -> i32;
    pub(crate) fn MonitorFromWindow(hwnd: Hwnd, dw_flags: u32) -> Hmonitor;
    pub(crate) fn GetMonitorInfoW(monitor: Hmonitor, monitor_info: *mut MonitorInfo) -> i32;
    pub(crate) fn DwmGetWindowAttribute(
        hwnd: Hwnd,
        attribute: u32,
        value: *mut c_void,
        value_size: u32,
    ) -> i32;
    pub(crate) fn SetWindowPos(
        hwnd: Hwnd,
        hwnd_insert_after: Hwnd,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    pub(crate) fn ShowWindow(hwnd: Hwnd, cmd_show: i32) -> i32;
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_windows_for_pid(hwnd: Hwnd, l_param: isize) -> i32 {
    let state = &mut *(l_param as *mut EnumWindowState);
    let mut process_id = 0u32;
    GetWindowThreadProcessId(hwnd, &mut process_id);
    if process_id != state.pid || IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    let Some(rect) = query_window_rect(hwnd) else {
        return 1;
    };
    let width = i64::from(rect.right - rect.left);
    let height = i64::from(rect.bottom - rect.top);
    if width <= 0 || height <= 0 {
        return 1;
    }
    let area = width * height;
    if area > state.best_area {
        state.best_area = area;
        state.best_hwnd = hwnd;
    }
    1
}

#[cfg(target_os = "windows")]
pub(crate) fn find_main_window_for_pid(pid: u32) -> Option<Hwnd> {
    let mut state = EnumWindowState {
        pid,
        best_hwnd: ptr::null_mut(),
        best_area: 0,
    };
    unsafe {
        EnumWindows(
            Some(enum_windows_for_pid),
            &mut state as *mut EnumWindowState as isize,
        );
    }
    (!state.best_hwnd.is_null()).then_some(state.best_hwnd)
}

#[cfg(target_os = "windows")]
pub(crate) fn process_has_visible_main_window(pid: u32) -> bool {
    find_main_window_for_pid(pid).is_some()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn process_has_visible_main_window(_pid: u32) -> bool {
    false
}

#[cfg(target_os = "windows")]
unsafe fn query_window_rect(hwnd: Hwnd) -> Option<Rect> {
    let mut rect = Rect::default();
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        &mut rect as *mut Rect as *mut c_void,
        size_of::<Rect>() as u32,
    ) != 0
        && GetWindowRect(hwnd, &mut rect) == 0
    {
        return None;
    }
    Some(rect)
}

#[cfg(target_os = "windows")]
unsafe fn monitor_from_window(hwnd: Hwnd, flags: u32) -> Option<Hmonitor> {
    let monitor = MonitorFromWindow(hwnd, flags);
    (!monitor.is_null()).then_some(monitor)
}

#[cfg(target_os = "windows")]
unsafe fn get_monitor_info(monitor: Hmonitor) -> Option<MonitorInfo> {
    let mut info = MonitorInfo {
        cb_size: size_of::<MonitorInfo>() as u32,
        ..MonitorInfo::default()
    };
    (GetMonitorInfoW(monitor, &mut info) != 0).then_some(info)
}

#[cfg(target_os = "windows")]
unsafe fn is_window_visible(hwnd: Hwnd) -> i32 {
    IsWindowVisible(hwnd)
}

#[cfg(target_os = "windows")]
pub(crate) fn hide_native_overlay_window(window: &tauri::WebviewWindow) {
    let Ok(overlay_hwnd) = window.hwnd() else {
        return;
    };
    let overlay_hwnd = overlay_hwnd.0 as Hwnd;
    unsafe {
        let _ = SetWindowPos(
            overlay_hwnd,
            ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_HIDEWINDOW,
        );
        let _ = ShowWindow(overlay_hwnd, SW_HIDE);
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn hide_native_overlay_window(_window: &tauri::WebviewWindow) {}


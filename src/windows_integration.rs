#[derive(Clone, Copy, Debug)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub scale_factor: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PopupAnchor {
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub work_area: WorkArea,
}

#[derive(Debug)]
pub struct SelectionCapture {
    pub text: String,
    pub source_window: isize,
    pub anchor: PopupAnchor,
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::mem;
    use std::ptr;
    use std::slice;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS, GlobalFree, HANDLE, HGLOBAL, HWND,
        LPARAM, POINT, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
    };
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, REG_DWORD, REG_NOTIFY_CHANGE_LAST_SET, REG_SZ,
        RRF_RT_REG_BINARY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegDeleteKeyValueW,
        RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, RegSetKeyValueW,
    };
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId,
        SetProcessWorkingSetSize,
    };
    use windows::Win32::UI::HiDpi::{
        GetDpiForMonitor, GetDpiForSystem, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
        KEYEVENTF_KEYUP, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, SendInput, UnregisterHotKey,
        VIRTUAL_KEY, VK_C, VK_CONTROL, VK_F9, VK_LBUTTON, VK_V,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, FindWindowExW, GetCursorPos, GetForegroundWindow, GetMessageW,
        GetWindowThreadProcessId, HWND_MESSAGE, IsWindow, MSG, PostMessageW, PostThreadMessageW,
        SM_CXSMICON, SetForegroundWindow, WM_APP, WM_HOTKEY, WM_QUIT, WM_RBUTTONUP,
    };
    use windows::core::{PCSTR, PCWSTR, w};

    use super::{PopupAnchor, SelectionCapture, WorkArea};
    use crate::messages::Message;

    const HOTKEY_ID: i32 = 0x4450;
    const SLINT_TRAY_MESSAGE: u32 = WM_APP + 1;
    const SLINT_TRAY_ID: usize = 1;
    const CF_UNICODETEXT: u32 = 13;
    const CLIPBOARD_RETRY_COUNT: usize = 20;
    const CLIPBOARD_RETRY_DELAY: Duration = Duration::from_millis(12);
    const PASTE_CHECK_DELAY: Duration = Duration::from_millis(200);

    const PERSONALIZE_KEY: PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");

    /// The size of small icons such as the tray icon, in physical pixels.
    pub fn small_icon_size() -> i32 {
        unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) }
    }

    /// Whether apps use the dark theme.
    pub fn windows_app_dark_mode() -> bool {
        personalize_value_is_zero(w!("AppsUseLightTheme"))
    }

    /// Whether the taskbar and tray use the dark theme. It can differ from the app theme.
    pub fn windows_system_dark_mode() -> bool {
        personalize_value_is_zero(w!("SystemUsesLightTheme"))
    }

    /// Makes native menus, such as the tray menu, dark or light. Windows has no
    /// public API for this; Explorer and Notepad use uxtheme's unnamed exports
    /// SetPreferredAppMode (ordinal 135) and FlushMenuThemes (ordinal 136).
    pub fn set_menu_dark_mode(dark: bool) {
        type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
        type FlushMenuThemes = unsafe extern "system" fn();
        const FORCE_DARK: i32 = 2;
        const FORCE_LIGHT: i32 = 3;

        let Ok(uxtheme) = (unsafe { LoadLibraryW(w!("uxtheme.dll")) }) else {
            return;
        };
        unsafe {
            if let Some(export) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) {
                let set_preferred_app_mode: SetPreferredAppMode = mem::transmute(export);
                set_preferred_app_mode(if dark { FORCE_DARK } else { FORCE_LIGHT });
            }
            if let Some(export) = GetProcAddress(uxtheme, PCSTR(136 as *const u8)) {
                let flush_menu_themes: FlushMenuThemes = mem::transmute(export);
                flush_menu_themes();
            }
        }
    }

    fn personalize_value_is_zero(name: PCWSTR) -> bool {
        let mut value = 1u32;
        let mut value_type = Default::default();
        let mut value_size = mem::size_of::<u32>() as u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PERSONALIZE_KEY,
                name,
                RRF_RT_REG_DWORD,
                Some(&mut value_type),
                Some((&mut value as *mut u32).cast()),
                Some(&mut value_size),
            )
        };

        result == ERROR_SUCCESS && value_type == REG_DWORD && value == 0
    }

    const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    // Task Manager's startup tab marks entries disabled here without removing them from Run.
    const STARTUP_APPROVED_KEY: PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
    const STARTUP_VALUE: PCWSTR = w!("DeepL Popup");

    /// Whether Windows starts the app when the user signs in.
    pub fn launch_at_startup() -> bool {
        let mut size = 0u32;
        let registered = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                STARTUP_VALUE,
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut size),
            )
        };
        if registered != ERROR_SUCCESS && registered != ERROR_MORE_DATA {
            return false;
        }

        // The first byte is even when the entry is enabled and odd when it is disabled.
        let mut approval = [0u8; 12];
        let mut approval_size = approval.len() as u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                STARTUP_APPROVED_KEY,
                STARTUP_VALUE,
                RRF_RT_REG_BINARY,
                None,
                Some(approval.as_mut_ptr().cast()),
                Some(&mut approval_size),
            )
        };
        result != ERROR_SUCCESS || approval_size == 0 || approval[0] & 1 == 0
    }

    pub fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
        let failed = || Message::LaunchAtStartupFailed.text();

        // A stale approval entry would keep the new registration disabled.
        let cleared =
            unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, STARTUP_APPROVED_KEY, STARTUP_VALUE) };
        if cleared != ERROR_SUCCESS && cleared != ERROR_FILE_NOT_FOUND {
            return Err(failed());
        }

        if !enabled {
            let removed = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, STARTUP_VALUE) };
            return if removed == ERROR_SUCCESS || removed == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(failed())
            };
        }

        let exe = std::env::current_exe().map_err(|_| failed())?;
        let command: Vec<u16> = format!("\"{}\"", exe.display())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let written = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                STARTUP_VALUE,
                REG_SZ.0,
                Some(command.as_ptr().cast()),
                (command.len() * mem::size_of::<u16>()) as u32,
            )
        };
        if written == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(failed())
        }
    }

    /// Calls `on_change` on a background thread whenever the theme settings
    /// change. The thread sleeps in the registry wait, so it costs nothing in between.
    pub fn watch_theme_changes<F>(on_change: F)
    where
        F: Fn() + Send + 'static,
    {
        let spawned = thread::Builder::new()
            .name("deepl-popup-theme".to_owned())
            .spawn(move || {
                let mut key = HKEY::default();
                let opened = unsafe {
                    RegOpenKeyExW(
                        HKEY_CURRENT_USER,
                        PERSONALIZE_KEY,
                        Some(0),
                        KEY_NOTIFY,
                        &mut key,
                    )
                };
                if opened != ERROR_SUCCESS {
                    return;
                }
                while unsafe {
                    RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, None, false)
                } == ERROR_SUCCESS
                {
                    on_change();
                }
                unsafe {
                    let _ = RegCloseKey(key);
                }
            });
        if let Err(error) = spawned {
            eprintln!("failed to start the theme watcher: {error}");
        }
    }

    pub fn show_tray_menu() -> Result<(), String> {
        let current_process_id = unsafe { GetCurrentProcessId() };
        let mut child_after = None;

        loop {
            let Ok(window) = (unsafe {
                FindWindowExW(
                    Some(HWND_MESSAGE),
                    child_after,
                    w!("SlintSystemTrayWindow"),
                    None,
                )
            }) else {
                break;
            };

            let mut process_id = 0;
            unsafe {
                GetWindowThreadProcessId(window, Some(&mut process_id));
            }
            if process_id == current_process_id {
                return unsafe {
                    PostMessageW(
                        Some(window),
                        SLINT_TRAY_MESSAGE,
                        WPARAM(SLINT_TRAY_ID),
                        LPARAM(WM_RBUTTONUP as isize),
                    )
                }
                .map_err(|error| format!("Failed to send the tray menu message: {error}"));
            }

            child_after = Some(window);
        }

        Err("Could not find the Slint tray window.".to_owned())
    }

    /// Activates `window` even when another process owns the foreground, so that
    /// the popup can close itself when it loses focus like the original does.
    pub fn bring_to_foreground(window: isize) {
        let window = HWND(window as *mut c_void);
        unsafe {
            if IsWindow(Some(window)).0 == 0 || GetForegroundWindow() == window {
                return;
            }
            let current_thread = GetCurrentThreadId();
            let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let attached = foreground_thread != 0
                && foreground_thread != current_thread
                && AttachThreadInput(current_thread, foreground_thread, true).as_bool();
            let _ = BringWindowToTop(window);
            let _ = SetForegroundWindow(window);
            if attached {
                let _ = AttachThreadInput(current_thread, foreground_thread, false);
            }
        }
    }

    pub struct HotkeyListener {
        thread_id: u32,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Drop for HotkeyListener {
        fn drop(&mut self) {
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    struct ClipboardOpenGuard;

    impl Drop for ClipboardOpenGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    pub fn start_hotkey_listener<F>(on_hotkey: F) -> Result<HotkeyListener, String>
    where
        F: Fn(Result<SelectionCapture, String>) + Send + 'static,
    {
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let handle = thread::Builder::new()
            .name("deepl-popup-hotkey".to_owned())
            .spawn(move || {
                let thread_id = unsafe { GetCurrentThreadId() };
                if let Err(error) = unsafe {
                    RegisterHotKey(None, HOTKEY_ID, MOD_CONTROL | MOD_NOREPEAT, VK_F9.0 as u32)
                } {
                    let _ = ready_sender.send(Err(Message::HotkeyRegistrationFailed {
                        error: error.to_string(),
                    }
                    .text()));
                    return;
                }

                if ready_sender.send(Ok(thread_id)).is_err() {
                    unsafe {
                        let _ = UnregisterHotKey(None, HOTKEY_ID);
                    }
                    return;
                }

                let mut message = MSG::default();
                loop {
                    let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
                    if result.0 <= 0 {
                        break;
                    }
                    if message.message == WM_HOTKEY && message.wParam.0 == HOTKEY_ID as usize {
                        on_hotkey(capture_selected_text());
                    }
                }

                unsafe {
                    let _ = UnregisterHotKey(None, HOTKEY_ID);
                }
            })
            .map_err(|error| format!("Failed to start the hotkey listener thread: {error}"))?;

        match ready_receiver.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(thread_id)) => Ok(HotkeyListener {
                thread_id,
                thread: Some(handle),
            }),
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(error) => Err(format!(
                "Failed to receive the hotkey initialization response: {error}"
            )),
        }
    }

    pub fn capture_selected_text() -> Result<SelectionCapture, String> {
        let source_window = unsafe { GetForegroundWindow() };
        if source_window.is_invalid() {
            return Err(Message::NoActiveWindow.text());
        }
        let anchor = current_anchor();
        let sequence_number = unsafe { GetClipboardSequenceNumber() };

        wait_for_hotkey_release();
        let send_result = if key_is_down(VK_CONTROL) {
            send_key_press(VK_C)
        } else {
            send_shortcut(VK_C)
        };

        let text = send_result.and_then(|_| {
            wait_for_new_clipboard_text(sequence_number, Duration::from_millis(900))
        })?;
        if text.trim().is_empty() {
            return Err(Message::NoSelectedText.text());
        }

        Ok(SelectionCapture {
            text,
            source_window: source_window.0 as isize,
            anchor,
        })
    }

    pub fn replace_selected_text(
        owner_window: isize,
        source_window: isize,
        text: &str,
    ) -> Result<(), String> {
        if text.is_empty() {
            return Err(Message::NoTranslationToReplace.text());
        }

        let source_window = HWND(source_window as *mut c_void);
        if unsafe { IsWindow(Some(source_window)) }.0 == 0 {
            return Err(Message::SourceWindowClosed.text());
        }

        set_clipboard_text(owner_window, text)?;

        if unsafe { SetForegroundWindow(source_window) }.0 == 0 {
            return Err(Message::SourceWindowUnreachable.text());
        }
        thread::sleep(Duration::from_millis(90));
        send_shortcut(VK_V)?;

        // Like the original, copy once more to check that the paste replaced the
        // selection. A read-only window keeps the selection, so the copy then
        // returns the original text instead of the translation.
        thread::sleep(PASTE_CHECK_DELAY);
        send_shortcut(VK_C)?;
        thread::sleep(PASTE_CHECK_DELAY);
        match read_clipboard_text() {
            Ok(Some(copied)) if !paste_succeeded(&copied, text) => {
                Err(Message::SourceReadOnly.text())
            }
            _ => Ok(()),
        }
    }

    /// Whether the text copied after pasting shows that the paste worked. Some
    /// editors (Notepad++, VS Code) copy the whole current line when nothing is
    /// selected, so the copy may contain the translation plus the rest of the line.
    pub(super) fn paste_succeeded(copied: &str, pasted: &str) -> bool {
        let normalize = |text: &str| text.replace("\r\n", "\n").trim().to_owned();
        let copied = normalize(copied);
        copied.is_empty() || copied.contains(&normalize(pasted))
    }

    pub fn copy_text(owner_window: isize, text: &str) -> Result<(), String> {
        set_clipboard_text(owner_window, text)
    }

    /// Lets Windows page out the memory the renderer keeps while no window is
    /// shown. The pages come back on demand when the popup opens again.
    pub fn trim_working_set() {
        unsafe {
            let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
        }
    }

    pub fn current_anchor() -> PopupAnchor {
        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&mut cursor) }.is_err() {
            cursor = POINT { x: 0, y: 0 };
        }
        PopupAnchor {
            cursor_x: cursor.x,
            cursor_y: cursor.y,
            work_area: work_area_for_point(cursor),
        }
    }

    pub fn left_button_down() -> bool {
        key_is_down(VK_LBUTTON)
    }

    pub fn cursor_is_in_rect(left: i32, top: i32, right: i32, bottom: i32) -> bool {
        let mut cursor = POINT::default();
        unsafe { GetCursorPos(&mut cursor) }.is_ok()
            && cursor.x >= left
            && cursor.x < right
            && cursor.y >= top
            && cursor.y < bottom
    }

    fn work_area_for_point(point: POINT) -> WorkArea {
        let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if monitor.is_invalid() || unsafe { GetMonitorInfoW(monitor, &mut info) }.0 == 0 {
            return WorkArea {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
                scale_factor: 1.0,
            };
        }

        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_err()
        {
            dpi_x = 96;
        }

        WorkArea {
            left: info.rcWork.left,
            top: info.rcWork.top,
            right: info.rcWork.right,
            bottom: info.rcWork.bottom,
            scale_factor: dpi_x as f32 / 96.0,
        }
    }

    fn wait_for_hotkey_release() {
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline && (key_is_down(VK_CONTROL) || key_is_down(VK_F9)) {
            thread::sleep(Duration::from_millis(8));
        }
    }

    fn key_is_down(key: VIRTUAL_KEY) -> bool {
        unsafe { (GetAsyncKeyState(key.0 as i32) as u16 & 0x8000) != 0 }
    }

    fn send_shortcut(key: VIRTUAL_KEY) -> Result<(), String> {
        let inputs = [
            keyboard_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
            keyboard_input(key, KEYBD_EVENT_FLAGS(0)),
            keyboard_input(key, KEYEVENTF_KEYUP),
            keyboard_input(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        send_inputs(&inputs)
    }

    fn send_key_press(key: VIRTUAL_KEY) -> Result<(), String> {
        let inputs = [
            keyboard_input(key, KEYBD_EVENT_FLAGS(0)),
            keyboard_input(key, KEYEVENTF_KEYUP),
        ];
        send_inputs(&inputs)
    }

    fn keyboard_input(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        }
    }

    fn send_inputs(inputs: &[INPUT]) -> Result<(), String> {
        let sent = unsafe { SendInput(inputs, mem::size_of::<INPUT>() as i32) };
        if sent == inputs.len() as u32 {
            Ok(())
        } else {
            Err(Message::KeyboardInputFailed.text())
        }
    }

    fn wait_for_new_clipboard_text(
        previous_sequence: u32,
        timeout: Duration,
    ) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        let mut clipboard_changed = false;

        while Instant::now() < deadline {
            if unsafe { GetClipboardSequenceNumber() } != previous_sequence {
                clipboard_changed = true;
                if let Ok(Some(text)) = read_clipboard_text() {
                    return Ok(text);
                }
            }
            thread::sleep(Duration::from_millis(12));
        }

        if clipboard_changed {
            Err(Message::SelectionNotText.text())
        } else {
            Err(Message::SelectionUnavailable.text())
        }
    }

    fn open_clipboard_with_retry(owner: Option<HWND>) -> Result<ClipboardOpenGuard, String> {
        let mut last_error = None;
        for _ in 0..CLIPBOARD_RETRY_COUNT {
            match unsafe { OpenClipboard(owner) } {
                Ok(()) => return Ok(ClipboardOpenGuard),
                Err(error) => last_error = Some(error),
            }
            thread::sleep(CLIPBOARD_RETRY_DELAY);
        }
        Err(Message::ClipboardFailed {
            error: last_error
                .map(|error| error.to_string())
                .unwrap_or_default(),
        }
        .text())
    }

    fn read_clipboard_text() -> Result<Option<String>, String> {
        let _clipboard = open_clipboard_with_retry(None)?;
        let Ok(handle) = (unsafe { GetClipboardData(CF_UNICODETEXT) }) else {
            return Ok(None);
        };
        let global = HGLOBAL(handle.0);
        let pointer = unsafe { GlobalLock(global) } as *const u16;
        if pointer.is_null() {
            return Err(clipboard_failed(""));
        }

        let units = unsafe { GlobalSize(global) } / mem::size_of::<u16>();
        let data = unsafe { slice::from_raw_parts(pointer, units) };
        let length = data.iter().position(|unit| *unit == 0).unwrap_or(units);
        let text = String::from_utf16_lossy(&data[..length]);
        unsafe {
            let _ = GlobalUnlock(global);
        }
        Ok(Some(text))
    }

    fn clipboard_failed(error: impl ToString) -> String {
        Message::ClipboardFailed {
            error: error.to_string(),
        }
        .text()
    }

    fn set_clipboard_text(owner_window: isize, text: &str) -> Result<(), String> {
        let owner_window = HWND(owner_window as *mut c_void);
        if unsafe { IsWindow(Some(owner_window)) }.0 == 0 {
            return Err(Message::ClipboardUnavailable.text());
        }
        let encoded: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let byte_count = encoded.len() * mem::size_of::<u16>();
        let bytes = unsafe { slice::from_raw_parts(encoded.as_ptr().cast::<u8>(), byte_count) };
        let global =
            unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.map_err(clipboard_failed)?;
        let target = unsafe { GlobalLock(global) } as *mut u8;
        if target.is_null() {
            unsafe {
                let _ = GlobalFree(Some(global));
            }
            return Err(clipboard_failed(""));
        }

        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
            let _ = GlobalUnlock(global);
        }

        let clipboard = match open_clipboard_with_retry(Some(owner_window)) {
            Ok(clipboard) => clipboard,
            Err(error) => {
                unsafe {
                    let _ = GlobalFree(Some(global));
                }
                return Err(error);
            }
        };
        if let Err(error) = unsafe { EmptyClipboard() } {
            drop(clipboard);
            unsafe {
                let _ = GlobalFree(Some(global));
            }
            return Err(clipboard_failed(error));
        }

        if let Err(error) = unsafe { SetClipboardData(CF_UNICODETEXT, Some(HANDLE(global.0))) } {
            unsafe {
                let _ = GlobalFree(Some(global));
            }
            return Err(clipboard_failed(error));
        }

        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{PopupAnchor, SelectionCapture, WorkArea};
    use crate::messages::Message;

    pub struct HotkeyListener;

    pub fn start_hotkey_listener<F>(_on_hotkey: F) -> Result<HotkeyListener, String>
    where
        F: Fn(Result<SelectionCapture, String>) + Send + 'static,
    {
        Err("The global Ctrl+F9 hotkey is supported only on Windows.".to_owned())
    }

    pub fn replace_selected_text(
        _owner_window: isize,
        _source_window: isize,
        _text: &str,
    ) -> Result<(), String> {
        Err("Replacing selected text is supported only on Windows.".to_owned())
    }

    pub fn copy_text(_owner_window: isize, _text: &str) -> Result<(), String> {
        Err("Clipboard copy is supported only on Windows.".to_owned())
    }

    pub fn show_tray_menu() -> Result<(), String> {
        Err("The tray menu is supported only on Windows.".to_owned())
    }

    pub fn bring_to_foreground(_window: isize) {}

    pub fn trim_working_set() {}

    pub fn current_anchor() -> PopupAnchor {
        PopupAnchor {
            cursor_x: 0,
            cursor_y: 0,
            work_area: WorkArea {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
                scale_factor: 1.0,
            },
        }
    }

    pub fn left_button_down() -> bool {
        false
    }

    pub fn cursor_is_in_rect(_left: i32, _top: i32, _right: i32, _bottom: i32) -> bool {
        true
    }

    pub fn windows_app_dark_mode() -> bool {
        false
    }

    pub fn windows_system_dark_mode() -> bool {
        false
    }

    pub fn set_menu_dark_mode(_dark: bool) {}

    pub fn watch_theme_changes<F>(_on_change: F)
    where
        F: Fn() + Send + 'static,
    {
    }

    pub fn launch_at_startup() -> bool {
        false
    }

    pub fn small_icon_size() -> i32 {
        16
    }

    pub fn set_launch_at_startup(_enabled: bool) -> Result<(), String> {
        Err(Message::LaunchAtStartupUnsupported.text())
    }
}

pub use platform::{
    bring_to_foreground, copy_text, current_anchor, cursor_is_in_rect, launch_at_startup,
    left_button_down, replace_selected_text, set_launch_at_startup, set_menu_dark_mode,
    show_tray_menu, small_icon_size, start_hotkey_listener, trim_working_set, watch_theme_changes,
    windows_app_dark_mode, windows_system_dark_mode,
};

#[cfg(all(test, windows))]
mod tests {
    use super::platform::paste_succeeded;

    #[test]
    fn paste_check_accepts_editors_that_copy_the_whole_line() {
        assert!(paste_succeeded("", "안녕"));
        assert!(paste_succeeded("안녕", "안녕"));
        assert!(paste_succeeded("앞 안녕 뒤\r\n", "안녕"));
        assert!(paste_succeeded("첫 줄\r\n둘째 줄\r\n", "첫 줄\n둘째 줄"));
        assert!(!paste_succeeded("Hello", "안녕"));
    }
}

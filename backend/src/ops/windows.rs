//! Windows：有窗口的发送 WM_CLOSE，控制台进程发送 Ctrl+C；强制结束用 TerminateProcess。

use std::time::Duration;

use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
};

use crate::console;
use crate::error::{is_invalid_parameter, AppError, AppResult};
use crate::model::CloseMethod;

const KILL_WAIT: Duration = Duration::from_secs(3);

/// 请求进程自行退出；既没有可见窗口也无法发送 Ctrl+C 时返回 None。
pub fn request_close(pid: u32) -> AppResult<Option<CloseMethod>> {
    Ok(if post_close_to_windows(pid)? {
        Some(CloseMethod::WindowClose)
    } else if console::spawn_ctrl_c(pid) {
        Some(CloseMethod::ConsoleCtrlC)
    } else {
        None
    })
}

/// 强制结束并等待退出；返回是否已退出。
pub fn terminate(pid: u32) -> AppResult<bool> {
    unsafe {
        let handle = match OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid) {
            Ok(h) => h,
            Err(e) if is_invalid_parameter(&e) => return Err(AppError::not_found(pid)),
            Err(e) => return Err(AppError::win32("OpenProcess", &e)),
        };
        let result = TerminateProcess(handle, 1);
        // 进程恰好正在退出时 TerminateProcess 会返回拒绝访问，此时句柄已触发，按已退出处理，不误报权限不足
        let exited = wait_handle(handle, if result.is_ok() { KILL_WAIT } else { Duration::ZERO });
        let _ = CloseHandle(handle);
        if exited {
            return Ok(true);
        }
        result.map_err(|e| AppError::win32("TerminateProcess", &e))?;
        Ok(false)
    }
}

/// 等待进程退出；进程已不存在视为已退出。
pub fn wait_exit(pid: u32, timeout: Duration) -> AppResult<bool> {
    unsafe {
        match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(h) => {
                let exited = wait_handle(h, timeout);
                let _ = CloseHandle(h);
                Ok(exited)
            }
            Err(e) if is_invalid_parameter(&e) => Ok(true),
            Err(e) => Err(AppError::win32("OpenProcess", &e)),
        }
    }
}

unsafe fn wait_handle(handle: HANDLE, timeout: Duration) -> bool {
    WaitForSingleObject(handle, timeout.as_millis() as u32) == WAIT_OBJECT_0
}

struct WindowSearch {
    pid: u32,
    windows: Vec<HWND>,
}

/// 控制台宿主替控制台程序创建的窗口。系统会把它的所属进程报告为控制台程序本身，但它不是程序的界面：
/// Windows Terminal（ConPTY）下的 PseudoConsoleWindow 收到 WM_CLOSE 毫无作用，
/// 传统 conhost 的 ConsoleWindowClass 收到 WM_CLOSE 会关掉整个终端窗口。这类进程应改发 Ctrl+C。
fn is_console_host_window(class: &str) -> bool {
    matches!(class, "PseudoConsoleWindow" | "ConsoleWindowClass")
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam.0 as *mut WindowSearch);
    let mut owner = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut owner));
    if owner == search.pid && IsWindowVisible(hwnd).as_bool() {
        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class).max(0) as usize;
        if !is_console_host_window(&String::from_utf16_lossy(&class[..len])) {
            search.windows.push(hwnd);
        }
    }
    BOOL(1)
}

/// 向目标进程的可见顶层窗口（不含控制台宿主窗口）发送 WM_CLOSE；没有窗口时返回 false。
/// 目标权限更高时消息会被 UIPI 拦下，返回拒绝访问，而不是当成“没有窗口”。
fn post_close_to_windows(pid: u32) -> AppResult<bool> {
    let mut search = WindowSearch { pid, windows: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(collect_window), LPARAM(&mut search as *mut _ as isize));
        let (mut posted, mut denied) = (false, false);
        for hwnd in &search.windows {
            match PostMessageW(Some(*hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) {
                Ok(()) => posted = true,
                Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => denied = true,
                // 窗口恰好已销毁等情况：当作没有窗口，继续尝试 Ctrl+C
                Err(_) => {}
            }
        }
        if denied && !posted {
            return Err(AppError::access_denied());
        }
        Ok(posted)
    }
}

#[cfg(test)]
mod tests {
    use super::is_console_host_window;

    #[test]
    fn console_host_windows_are_skipped() {
        assert!(is_console_host_window("PseudoConsoleWindow"));
        assert!(is_console_host_window("ConsoleWindowClass"));
    }

    #[test]
    fn app_windows_are_kept() {
        assert!(!is_console_host_window("Chrome_WidgetWin_1"));
        assert!(!is_console_host_window("CASCADIA_HOSTING_WINDOW_CLASS"));
        assert!(!is_console_host_window(""));
    }
}

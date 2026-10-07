//! Windows：有窗口的发送 WM_CLOSE，控制台进程发送 Ctrl+C；强制结束用 TerminateProcess。

use std::time::Duration;

use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
};

use crate::console;
use crate::error::{is_invalid_parameter, AppError, AppResult};
use crate::model::CloseMethod;

const KILL_WAIT: Duration = Duration::from_secs(3);

/// 请求进程自行退出；既没有可见窗口也无法发送 Ctrl+C 时返回 None。
pub fn request_close(pid: u32) -> AppResult<Option<CloseMethod>> {
    Ok(if post_close_to_windows(pid) {
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
        let exited = result.is_ok() && wait_handle(handle, KILL_WAIT);
        let _ = CloseHandle(handle);
        result.map_err(|e| AppError::win32("TerminateProcess", &e))?;
        Ok(exited)
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

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam.0 as *mut WindowSearch);
    let mut owner = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut owner));
    if owner == search.pid && IsWindowVisible(hwnd).as_bool() {
        search.windows.push(hwnd);
    }
    BOOL(1)
}

/// 向目标进程的可见顶层窗口发送 WM_CLOSE；没有窗口时返回 false。
fn post_close_to_windows(pid: u32) -> bool {
    let mut search = WindowSearch { pid, windows: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(collect_window), LPARAM(&mut search as *mut _ as isize));
        let mut posted = false;
        for hwnd in &search.windows {
            posted |= PostMessageW(Some(*hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).is_ok();
        }
        posted
    }
}

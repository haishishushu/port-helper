//! 进程操作：身份校验、正常关闭、强制结束。

use std::time::{Duration, Instant};

use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
};

use crate::console;
use crate::error::{is_invalid_parameter, AppError, AppResult, ErrorCode};
use crate::model::{CloseMethod, CloseOutcome, CloseResult, KillResult, OccupantKind};
use crate::process::classify::{classify, ClassifyInput};
use crate::process;

const KILL_WAIT: Duration = Duration::from_secs(3);
const MIN_CLOSE_TIMEOUT_MS: u32 = 1_000;
const MAX_CLOSE_TIMEOUT_MS: u32 = 60_000;

/// 操作前的最终安全校验：保护进程、关键进程确认、PID 复用校验。
/// 只读取目标进程的名称与启动时间，不加载全量进程快照和服务表（prd/todo/perf-todo.md P1-5）。
pub fn guard(pid: u32, expected_start: u64, confirm_critical: bool) -> AppResult<()> {
    let name = match pid {
        0 => "System Idle Process".to_string(),
        4 => "System".to_string(),
        _ => match process::image_name(pid) {
            Ok(Some(name)) => name,
            Ok(None) => return Err(AppError::not_found(pid)),
            // 受保护进程可能读不到映像路径，交给后面的启动时间校验兜底
            Err(_) => String::new(),
        },
    };
    let c = classify(&ClassifyInput {
        pid,
        name: &name,
        exe_path: None,
        command_line: None,
        services: &[],
        port_hint: None,
        self_pid: std::process::id(),
    });
    match c.kind {
        OccupantKind::Idle | OccupantKind::System | OccupantKind::SelfProcess => {
            let reason = c.blocked_reason.unwrap_or_else(|| "该进程受保护".into());
            return Err(AppError::new(ErrorCode::ProtectedProcess, reason));
        }
        OccupantKind::Critical if !confirm_critical => {
            return Err(AppError::new(
                ErrorCode::CriticalConfirmRequired,
                format!("{name} 是 Windows 关键进程，需要二次确认"),
            ));
        }
        _ => {}
    }
    match process::start_time(pid)? {
        None => Err(AppError::not_found(pid)),
        Some(actual) if actual != expected_start => Err(AppError::new(
            ErrorCode::ProcessChanged,
            format!("PID {pid} 已被其他进程复用，操作已取消"),
        )),
        Some(_) => Ok(()),
    }
}

pub fn close(pid: u32, start_time: u64, timeout_ms: u32, confirm_critical: bool) -> AppResult<CloseResult> {
    guard(pid, start_time, confirm_critical)?;
    let started = Instant::now();
    let timeout = timeout_ms.clamp(MIN_CLOSE_TIMEOUT_MS, MAX_CLOSE_TIMEOUT_MS);

    let method = if post_close_to_windows(pid) {
        CloseMethod::WindowClose
    } else if console::spawn_ctrl_c(pid) {
        CloseMethod::ConsoleCtrlC
    } else {
        log::info!("pid {pid}: 无可见窗口且无法发送 Ctrl+C");
        return Ok(CloseResult { outcome: CloseOutcome::Unsupported, method: CloseMethod::None, elapsed_ms: 0 });
    };

    let remaining = Duration::from_millis(timeout as u64).saturating_sub(started.elapsed());
    let outcome = if wait_exit(pid, remaining)? { CloseOutcome::Exited } else { CloseOutcome::Timeout };
    log::info!("pid {pid}: 正常关闭 {:?} / {:?}", method, outcome);
    Ok(CloseResult { outcome, method, elapsed_ms: started.elapsed().as_millis() as u64 })
}

pub fn kill(pid: u32, start_time: u64, confirm_critical: bool) -> AppResult<KillResult> {
    guard(pid, start_time, confirm_critical)?;
    let started = Instant::now();
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
        log::info!("pid {pid}: 强制结束，已退出 = {exited}");
        Ok(KillResult { exited, elapsed_ms: started.elapsed().as_millis() as u64 })
    }
}

/// 等待进程退出；进程已不存在视为已退出。
fn wait_exit(pid: u32, timeout: Duration) -> AppResult<bool> {
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

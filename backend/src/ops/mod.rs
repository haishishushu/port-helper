//! 进程操作：身份校验、正常关闭、强制结束。发送关闭请求与结束进程按平台实现。

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::time::{Duration, Instant};

#[cfg(unix)]
use self::unix as sys;
#[cfg(windows)]
use self::windows as sys;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::{CloseMethod, CloseOutcome, CloseResult, KillResult, OccupantKind};
use crate::process::classify::{classify, ClassifyInput};
use crate::process;

const MIN_CLOSE_TIMEOUT_MS: u32 = 1_000;
const MAX_CLOSE_TIMEOUT_MS: u32 = 60_000;

/// 操作前的最终安全校验：保护进程、关键进程确认、PID 复用校验。
/// 只读取目标进程的名称与启动时间，不加载全量进程快照和服务表（prd/todo/perf-todo.md P1-5）。
pub fn guard(pid: u32, expected_start: u64, confirm_critical: bool) -> AppResult<()> {
    let name = match process::pseudo_name(pid) {
        Some(name) => name.to_string(),
        None => match process::image_name(pid) {
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
            let label = if cfg!(windows) { " Windows " } else { "系统" };
            return Err(AppError::new(
                ErrorCode::CriticalConfirmRequired,
                format!("{name} 是{label}关键进程，需要二次确认"),
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

    let Some(method) = sys::request_close(pid)? else {
        log::info!("pid {pid}: 无法发送关闭请求");
        return Ok(CloseResult { outcome: CloseOutcome::Unsupported, method: CloseMethod::None, elapsed_ms: 0 });
    };

    let remaining = Duration::from_millis(timeout as u64).saturating_sub(started.elapsed());
    let outcome = if sys::wait_exit(pid, remaining)? { CloseOutcome::Exited } else { CloseOutcome::Timeout };
    log::info!("pid {pid}: 正常关闭 {:?} / {:?}", method, outcome);
    Ok(CloseResult { outcome, method, elapsed_ms: started.elapsed().as_millis() as u64 })
}

pub fn kill(pid: u32, start_time: u64, confirm_critical: bool) -> AppResult<KillResult> {
    guard(pid, start_time, confirm_critical)?;
    let started = Instant::now();
    let exited = sys::terminate(pid)?;
    log::info!("pid {pid}: 强制结束，已退出 = {exited}");
    Ok(KillResult { exited, elapsed_ms: started.elapsed().as_millis() as u64 })
}

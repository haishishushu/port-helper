//! Tauri Commands：只做参数校验与线程调度，业务逻辑在 query / ops / privilege 中。

use std::time::Instant;

use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::model::{CloseResult, KillResult, PidQueryResult, PortQueryResult, PrivilegeInfo};
use crate::process::ProcessCatalog;
use crate::{net, ops, privilege, query};

/// 系统调用都可能阻塞，统一放到阻塞线程池执行。
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::internal(format!("后台任务失败：{e}")))?
}

#[tauri::command]
pub async fn query_port(port: u32, include_all_states: bool) -> AppResult<PortQueryResult> {
    let port = query::validate_port(port)?;
    blocking(move || {
        let started = Instant::now();
        let all = net::list_all()?;
        let pids: Vec<u32> = all.iter().filter(|b| b.local_port == port).map(|b| b.pid).collect();
        let catalog = ProcessCatalog::load(&pids);
        let mut result = query::port_result(port, all, include_all_states, &catalog);
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn query_pid(pid: u32, include_all_states: bool) -> AppResult<PidQueryResult> {
    blocking(move || {
        let started = Instant::now();
        let all = net::list_all()?;
        let catalog = ProcessCatalog::load(&[pid]);
        let mut result = query::pid_result(pid, all, include_all_states, &catalog)?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn close_process(pid: u32, start_time: u64, timeout_ms: u32, confirm_critical: bool) -> AppResult<CloseResult> {
    blocking(move || ops::close(pid, start_time, timeout_ms, confirm_critical)).await
}

#[tauri::command]
pub async fn kill_process(pid: u32, start_time: u64, confirm_critical: bool) -> AppResult<KillResult> {
    blocking(move || ops::kill(pid, start_time, confirm_critical)).await
}

#[tauri::command]
pub fn get_privilege() -> PrivilegeInfo {
    PrivilegeInfo { elevated: privilege::is_elevated() }
}

#[tauri::command]
pub fn relaunch_as_admin(app: AppHandle) -> AppResult<()> {
    privilege::relaunch_as_admin()?;
    app.exit(0);
    Ok(())
}

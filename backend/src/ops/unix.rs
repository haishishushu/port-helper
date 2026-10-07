//! macOS / Linux：正常关闭发送 SIGTERM，强制结束发送 SIGKILL，通过轮询判断进程是否已退出。

use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::model::CloseMethod;
use crate::process;

const KILL_WAIT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

fn send_signal(pid: u32, signal: libc::c_int) -> AppResult<()> {
    let pid_t = libc::pid_t::try_from(pid).map_err(|_| AppError::not_found(pid))?;
    if unsafe { libc::kill(pid_t, signal) } == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Err(AppError::not_found(pid));
    }
    Err(AppError::os("kill", &err))
}

/// 发送 SIGTERM 请求进程自行退出（等同于在终端里执行 `kill <pid>`）。
pub fn request_close(pid: u32) -> AppResult<Option<CloseMethod>> {
    send_signal(pid, libc::SIGTERM)?;
    Ok(Some(CloseMethod::Signal))
}

/// 发送 SIGKILL 并等待退出；返回是否已退出。
pub fn terminate(pid: u32) -> AppResult<bool> {
    send_signal(pid, libc::SIGKILL)?;
    wait_exit(pid, KILL_WAIT)
}

/// 轮询等待进程退出；进程已不存在或已成为僵尸进程都视为已退出。
pub fn wait_exit(pid: u32, timeout: Duration) -> AppResult<bool> {
    let started = Instant::now();
    loop {
        if process::start_time(pid)?.is_none() {
            return Ok(true);
        }
        if started.elapsed() >= timeout {
            return Ok(false);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

//! macOS / Linux：单个进程的父 PID、名称、启动时间都通过 sysinfo 只刷新该进程读取。
//!
//! 启动时间精度为秒（换算成毫秒返回），同一 PID 在一秒内被复用的概率可以忽略。

use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System};

use crate::error::AppResult;

/// PID 0 不对应真实进程：表示读不到所属进程的套接字（其他用户的进程或内核持有的连接）
pub fn pseudo_name(pid: u32) -> Option<&'static str> {
    (pid == 0).then_some("未知进程")
}

/// 只刷新单个进程；进程不存在或已是僵尸进程（已退出、等待父进程回收）时返回 None。
fn with_process<T>(pid: u32, f: impl FnOnce(&sysinfo::Process) -> T) -> Option<T> {
    if pid == 0 {
        return None;
    }
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::nothing(),
    );
    let p = sys.process(Pid::from_u32(pid))?;
    (p.status() != ProcessStatus::Zombie).then(|| f(p))
}

pub fn parent_pid(pid: u32) -> Option<u32> {
    with_process(pid, |p| p.parent().map(|pp| pp.as_u32())).flatten().filter(|&pp| pp != 0)
}

/// 进程名。进程不存在返回 `Ok(None)`。
pub fn image_name(pid: u32) -> AppResult<Option<String>> {
    Ok(with_process(pid, |p| p.name().to_string_lossy().into_owned()))
}

/// 进程启动时间（Unix 毫秒）。进程不存在或已退出返回 `Ok(None)`。
pub fn start_time(pid: u32) -> AppResult<Option<u64>> {
    Ok(with_process(pid, |p| p.start_time()).filter(|&s| s > 0).map(|s| s * 1000))
}

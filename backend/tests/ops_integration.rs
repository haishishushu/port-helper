//! 集成测试：对自己启动的子进程执行强制结束、PID 复用校验和保护规则。

use std::process::{Child, Command};

use port_helper_lib::error::ErrorCode;
use port_helper_lib::{ops, process};

#[cfg(windows)]
fn spawn_sleeper() -> Child {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("ping")
        .args(["-n", "30", "127.0.0.1"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .expect("启动测试子进程失败")
}

#[cfg(unix)]
fn spawn_sleeper() -> Child {
    Command::new("sleep").arg("30").spawn().expect("启动测试子进程失败")
}

#[test]
fn kill_terminates_child_after_identity_check() {
    let mut child = spawn_sleeper();
    let pid = child.id();
    let start = process::start_time(pid).unwrap().expect("应能读取子进程启动时间");

    let result = ops::kill(pid, start, false).unwrap();
    assert!(result.exited);
    assert!(child.try_wait().unwrap().is_some(), "子进程应已退出");
}

#[test]
fn rejects_reused_pid() {
    let mut child = spawn_sleeper();
    let pid = child.id();
    let start = process::start_time(pid).unwrap().unwrap();

    let err = ops::kill(pid, start + 1, false).unwrap_err();
    assert_eq!(err.code, ErrorCode::ProcessChanged);
    assert!(child.try_wait().unwrap().is_none(), "校验失败时不应结束进程");
    let _ = child.kill();
}

#[test]
fn protects_system_and_self() {
    // Windows：PID 4 是内核；macOS / Linux：PID 1 是 launchd / systemd
    let system_pid = if cfg!(windows) { 4 } else { 1 };
    assert_eq!(ops::kill(system_pid, 0, true).unwrap_err().code, ErrorCode::ProtectedProcess);
    assert_eq!(ops::kill(0, 0, true).unwrap_err().code, ErrorCode::ProtectedProcess);
    assert_eq!(ops::kill(std::process::id(), 0, true).unwrap_err().code, ErrorCode::ProtectedProcess);
}

/// 正常关闭（SIGTERM）。Windows 的 Ctrl+C 会发给整个控制台进程组，会波及测试进程自身，不在此测试
#[test]
#[cfg(unix)]
fn close_requests_child_to_exit() {
    let mut child = spawn_sleeper();
    let pid = child.id();
    let start = process::start_time(pid).unwrap().expect("应能读取子进程启动时间");

    let result = ops::close(pid, start, 5_000, false).unwrap();
    assert_eq!(result.outcome, port_helper_lib::model::CloseOutcome::Exited, "子进程应已退出：{result:?}");
    let _ = child.wait();
}

#[test]
fn missing_process_is_reported() {
    let mut child = spawn_sleeper();
    let pid = child.id();
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(ops::kill(pid, 0, false).unwrap_err().code, ErrorCode::ProcessNotFound);
}

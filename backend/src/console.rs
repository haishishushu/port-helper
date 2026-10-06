//! 向控制台进程发送 Ctrl+C。
//!
//! 一个进程同一时间只能附着一个控制台，为了不影响 port-helper 自身（开发模式下有控制台），
//! 这里启动一个无控制台的辅助进程 `port-helper.exe --ctrl-c <pid>` 去完成附着与发送。

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

use windows::Win32::System::Console::{
    AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler, ATTACH_PARENT_PROCESS,
    CTRL_C_EVENT,
};
use windows::Win32::System::Threading::DETACHED_PROCESS;

pub const HELPER_FLAG: &str = "--ctrl-c";
const HELPER_TIMEOUT: Duration = Duration::from_secs(3);

/// 在 main 最开始调用：若以辅助模式启动，则发送 Ctrl+C 后直接退出进程。
pub fn run_helper_if_requested() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == HELPER_FLAG {
        let code = match args[2].parse::<u32>() {
            Ok(pid) if send_ctrl_c(pid) => 0,
            _ => 1,
        };
        std::process::exit(code);
    }
}

/// 开发模式下附加到父进程（cargo / tauri dev）的控制台以输出日志。
/// 以管理员身份重新启动时父进程没有控制台，调用失败即忽略，不会弹出窗口。
pub fn attach_parent_console() {
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn send_ctrl_c(pid: u32) -> bool {
    unsafe {
        let _ = FreeConsole();
        if AttachConsole(pid).is_err() {
            return false;
        }
        // 屏蔽辅助进程自身对 Ctrl+C 的处理，避免被一起结束
        let _ = SetConsoleCtrlHandler(None, true);
        // 进程组 0 = 该控制台上的全部进程，效果等同于用户在终端按 Ctrl+C
        let sent = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).is_ok();
        let _ = FreeConsole();
        sent
    }
}

/// 启动辅助进程发送 Ctrl+C；返回是否发送成功（目标没有控制台时为 false）。
pub fn spawn_ctrl_c(pid: u32) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let Ok(mut child) = Command::new(exe)
        .args([HELPER_FLAG, &pid.to_string()])
        .creation_flags(DETACHED_PROCESS.0)
        .spawn()
    else {
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if started.elapsed() < HELPER_TIMEOUT => std::thread::sleep(Duration::from_millis(30)),
            _ => {
                let _ = child.kill();
                return false;
            }
        }
    }
}

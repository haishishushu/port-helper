//! 管理员（root）权限检测与以管理员身份重新启动。

#[cfg(unix)]
use crate::error::AppResult;
use crate::error::{AppError, ErrorCode};

/// 提示文案里的权限称呼
pub const ADMIN_TERM: &str = if cfg!(target_os = "linux") { "root" } else { "管理员" };

/// 以管理员身份重新启动时传给新实例的参数：`--wait-exit <旧实例 PID>`。
/// 新实例先等旧实例退出再初始化，否则会被单实例检查当成重复启动，唤出即将退出的旧窗口后自己退出。
pub const WAIT_EXIT_FLAG: &str = "--wait-exit";
const WAIT_EXIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn wait_exit_pid(args: &[String]) -> Option<u32> {
    args.windows(2).find(|w| w[0] == WAIT_EXIT_FLAG).and_then(|w| w[1].parse().ok())
}

/// 在 main 中、启动界面之前调用：带有 `--wait-exit` 参数时等旧实例退出（最多 10 秒）。
pub fn wait_for_previous_instance() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pid) = wait_exit_pid(&args) {
        crate::ops::wait_exited(pid, WAIT_EXIT_TIMEOUT);
    }
}

fn cancelled() -> AppError {
    AppError::new(ErrorCode::UacCancelled, "已取消提升权限，继续以普通权限运行")
}

#[cfg(windows)]
pub use self::windows_impl::{is_elevated, relaunch_as_admin};

#[cfg(windows)]
mod windows_impl {
    use std::os::windows::ffi::OsStrExt;

    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    use super::cancelled;
    use crate::error::{AppError, AppResult, ErrorCode};

    /// ShellExecute 返回值 ≤ 32 表示失败；SE_ERR_ACCESSDENIED(5) 表示用户取消了 UAC。
    const SE_ERR_ACCESSDENIED: isize = 5;

    pub fn is_elevated() -> bool {
        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            )
            .is_ok();
            let _ = CloseHandle(token);
            ok && elevation.TokenIsElevated != 0
        }
    }

    /// 以管理员身份启动新实例；调用方在成功后退出当前进程。
    pub fn relaunch_as_admin() -> AppResult<()> {
        let exe = std::env::current_exe().map_err(|e| AppError::internal(e.to_string()))?;
        let wide: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let params: Vec<u16> =
            format!("{} {}", super::WAIT_EXIT_FLAG, std::process::id()).encode_utf16().chain(std::iter::once(0)).collect();
        let result = unsafe {
            ShellExecuteW(None, w!("runas"), PCWSTR(wide.as_ptr()), PCWSTR(params.as_ptr()), PCWSTR::null(), SW_SHOWNORMAL)
        };
        let code = result.0 as isize;
        match code {
            c if c > 32 => Ok(()),
            SE_ERR_ACCESSDENIED => Err(cancelled()),
            c => Err(AppError::new(ErrorCode::SystemApiFailed, format!("ShellExecuteW 调用失败（{c}）"))),
        }
    }
}

#[cfg(unix)]
pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// 要以 root 重新启动的程序路径；AppImage 运行时 current_exe 位于只有当前用户可访问的临时挂载目录，
/// 所以改用 AppImage 文件本身。
#[cfg(unix)]
fn relaunch_target() -> AppResult<String> {
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        return Ok(appimage);
    }
    let exe = std::env::current_exe().map_err(|e| AppError::internal(e.to_string()))?;
    Ok(exe.to_string_lossy().into_owned())
}

/// macOS：通过 AppleScript 弹出系统的管理员密码框，在后台以 root 启动新实例后立即返回。
#[cfg(target_os = "macos")]
pub fn relaunch_as_admin() -> AppResult<()> {
    let exe = relaunch_target()?;
    let shell = format!("'{}' {WAIT_EXIT_FLAG} {} > /dev/null 2>&1 &", exe.replace('\'', r"'\''"), std::process::id());
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        shell.replace('\\', r"\\").replace('"', "\\\"")
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| AppError::new(ErrorCode::SystemApiFailed, format!("无法启动 osascript：{e}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    // 用户在密码框点了“取消”时 AppleScript 返回 -128
    if stderr.contains("-128") {
        return Err(cancelled());
    }
    Err(AppError::new(ErrorCode::SystemApiFailed, format!("以管理员身份启动失败：{}", stderr.trim())))
}

/// Linux：通过 pkexec 弹出系统的认证框，在后台以 root 启动新实例后立即返回。
/// pkexec 会清空环境变量，所以显式传入图形会话需要的变量。
#[cfg(target_os = "linux")]
pub fn relaunch_as_admin() -> AppResult<()> {
    let exe = relaunch_target()?;
    let mut cmd = std::process::Command::new("pkexec");
    cmd.arg("env");
    for key in ["DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "XAUTHORITY"] {
        if let Ok(value) = std::env::var(key) {
            cmd.arg(format!("{key}={value}"));
        }
    }
    if std::env::var_os("XAUTHORITY").is_none() {
        if let Some(home) = std::env::var_os("HOME") {
            let xauth = std::path::Path::new(&home).join(".Xauthority");
            if xauth.exists() {
                cmd.arg(format!("XAUTHORITY={}", xauth.to_string_lossy()));
            }
        }
    }
    let pid = std::process::id().to_string();
    cmd.args(["sh", "-c", "nohup \"$0\" \"$1\" \"$2\" > /dev/null 2>&1 &", exe.as_str(), WAIT_EXIT_FLAG, &pid]);
    let status = match cmd.status() {
        Ok(status) => status,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::new(
                ErrorCode::SystemApiFailed,
                "未找到 pkexec，请在终端中使用 sudo 运行 port-helper",
            ))
        }
        Err(e) => return Err(AppError::new(ErrorCode::SystemApiFailed, format!("无法启动 pkexec：{e}"))),
    };
    match status.code() {
        Some(0) => Ok(()),
        // 126：用户关闭了认证框；127：认证失败或未获授权
        Some(126) | Some(127) => Err(cancelled()),
        code => Err(AppError::new(ErrorCode::SystemApiFailed, format!("pkexec 退出码 {code:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::{wait_exit_pid, WAIT_EXIT_FLAG};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn wait_exit_pid_reads_flag_value() {
        assert_eq!(wait_exit_pid(&args(&["port-helper.exe", WAIT_EXIT_FLAG, "1234"])), Some(1234));
    }

    #[test]
    fn wait_exit_pid_ignores_missing_or_invalid_value() {
        assert_eq!(wait_exit_pid(&args(&["port-helper.exe"])), None);
        assert_eq!(wait_exit_pid(&args(&["port-helper.exe", WAIT_EXIT_FLAG])), None);
        assert_eq!(wait_exit_pid(&args(&["port-helper.exe", WAIT_EXIT_FLAG, "abc"])), None);
    }
}

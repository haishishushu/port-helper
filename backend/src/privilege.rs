//! 管理员（root）权限检测与以管理员身份重新启动。

#[cfg(unix)]
use crate::error::AppResult;
use crate::error::{AppError, ErrorCode};

/// 提示文案里的权限称呼
pub const ADMIN_TERM: &str = if cfg!(target_os = "linux") { "root" } else { "管理员" };

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
        let result = unsafe {
            ShellExecuteW(None, w!("runas"), PCWSTR(wide.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
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
    let shell = format!("'{}' > /dev/null 2>&1 &", exe.replace('\'', r"'\''"));
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
    cmd.args(["sh", "-c", "nohup \"$0\" > /dev/null 2>&1 &", exe.as_str()]);
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

//! 管理员权限检测与以管理员身份重新启动。

use std::os::windows::ffi::OsStrExt;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

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
        SE_ERR_ACCESSDENIED => Err(AppError::new(ErrorCode::UacCancelled, "已取消提升权限，继续以普通权限运行")),
        c => Err(AppError::new(ErrorCode::SystemApiFailed, format!("ShellExecuteW 调用失败（{c}）"))),
    }
}

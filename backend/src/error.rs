use serde::Serialize;
use ts_rs::TS;
#[cfg(windows)]
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER};

/// 前端可识别的错误码，与 `prd/impl/implementation.md` 5.3 节一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    InvalidPort,
    InvalidPid,
    ProcessNotFound,
    ProcessChanged,
    ProtectedProcess,
    CriticalConfirmRequired,
    AccessDenied,
    UacCancelled,
    SystemApiFailed,
    Internal,
}

/// 所有 Command 的统一错误结构：`{ code, message }`。
#[derive(Debug, Clone, Serialize, TS, thiserror::Error)]
#[error("{message}")]
#[ts(export)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn not_found(pid: u32) -> Self {
        Self::new(ErrorCode::ProcessNotFound, format!("PID {pid} 不存在或已退出"))
    }

    pub fn access_denied() -> Self {
        Self::new(ErrorCode::AccessDenied, format!("拒绝访问，需要{}权限", crate::privilege::ADMIN_TERM))
    }

    /// 把 Win32 错误转换为业务错误；权限不足单独识别。
    #[cfg(windows)]
    pub fn win32(api: &str, err: &windows::core::Error) -> Self {
        if err.code() == ERROR_ACCESS_DENIED.to_hresult() {
            return Self::access_denied();
        }
        Self::new(
            ErrorCode::SystemApiFailed,
            format!("{api} 调用失败（0x{:08X}）", err.code().0 as u32),
        )
    }

    /// Win32 返回码风格（非 HRESULT）的失败。
    #[cfg(windows)]
    pub fn win32_code(api: &str, code: u32) -> Self {
        if code == ERROR_ACCESS_DENIED.0 {
            return Self::access_denied();
        }
        Self::new(ErrorCode::SystemApiFailed, format!("{api} 调用失败（{code}）"))
    }

    /// 把 Unix 系统调用错误转换为业务错误；权限不足单独识别。
    #[cfg(unix)]
    pub fn os(api: &str, err: &std::io::Error) -> Self {
        match err.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => Self::access_denied(),
            _ => Self::new(ErrorCode::SystemApiFailed, format!("{api} 调用失败（{err}）")),
        }
    }
}

/// OpenProcess 对不存在的 PID 返回 ERROR_INVALID_PARAMETER。
#[cfg(windows)]
pub fn is_invalid_parameter(err: &windows::core::Error) -> bool {
    err.code() == ERROR_INVALID_PARAMETER.to_hresult()
}

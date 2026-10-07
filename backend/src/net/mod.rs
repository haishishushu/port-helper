//! 读取系统 TCP / UDP 端口表（IPv4 + IPv6），按平台分别实现，对外只暴露 `list_all`。

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use self::unix::list_all;
#[cfg(windows)]
pub use self::windows::list_all;

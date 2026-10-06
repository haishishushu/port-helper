//! WebView2 内存目标级别（prd/todo/perf-todo.md P1-1）。
//!
//! 窗口最小化时把 WebView2 的 MemoryUsageTargetLevel 设为 Low，让渲染进程与 GPU 进程
//! 释放可回收的内存；恢复显示时设回 Normal。需要 WebView2 Runtime 114+，更低版本调用无效果。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Manager, Window};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
};
use windows::core::Interface;

/// 当前是否处于低内存级别，避免每次窗口尺寸变化都重复调用
static LOW: AtomicBool = AtomicBool::new(false);

/// 在窗口尺寸变化（含最小化 / 还原）时调用。
pub fn sync_with_window(window: &Window) {
    let low = window.is_minimized().unwrap_or(false);
    if LOW.swap(low, Ordering::Relaxed) == low {
        return;
    }
    let Some(webview) = window.app_handle().get_webview_window(window.label()) else { return };
    let _ = webview.with_webview(move |platform| unsafe {
        let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
        let result = platform
            .controller()
            .CoreWebView2()
            .and_then(|core| core.cast::<ICoreWebView2_19>())
            .and_then(|core| core.SetMemoryUsageTargetLevel(level));
        match result {
            Ok(()) => log::debug!("WebView2 内存级别 → {}", if low { "Low" } else { "Normal" }),
            Err(e) => log::warn!("设置 WebView2 内存级别失败：{e}"),
        }
    });
}

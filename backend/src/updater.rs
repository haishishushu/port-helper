//! 应用自更新：走 tauri-plugin-updater，更新端点与签名公钥配置在 tauri.conf.json，
//! CI 发布时用私钥对安装包签名，并在 Release 附上 latest.json。
//!
//! 前端在启动 1 秒后静默检查一次，设置页可手动检查；`download_app_update` 下载并校验签名后
//! 把安装包留在内存里，用户点“安装并重启”时 `install_app_update` 直接使用，不再重复下载。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, Updater, UpdaterExt};
use ts_rs::TS;
use url::Url;

use crate::error::{AppError, AppResult};

/// 下载进度事件名，与前端 `hooks/use-update.tsx` 一致
const PROGRESS_EVENT: &str = "update-download-progress";
/// 进度事件的最小间隔字节数，避免每个数据块都触发一次前端重绘
const PROGRESS_STEP: u64 = 128 * 1024;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateCheck {
    pub available: bool,
    pub current_version: String,
    /// 没有新版本时为空字符串
    pub available_version: String,
    /// 更新说明；发布时未提供则为 null
    pub notes: Option<String>,
    /// 发布时间（RFC 3339）
    pub pub_date: Option<String>,
}

/// 下载进度事件负载；拿到 content-length 之前 total 为 null
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

struct PreparedUpdate {
    update: Update,
    bytes: Vec<u8>,
}

/// 已下载并通过签名校验、等待安装的更新包
#[derive(Default)]
pub struct UpdateState {
    prepared: Mutex<Option<PreparedUpdate>>,
    downloading: AtomicBool,
}

/// 下载结束（成功、失败或提前返回）时清除“下载中”标记
struct DownloadGuard<'a>(&'a AtomicBool);

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn update_check(update: &Update) -> UpdateCheck {
    UpdateCheck {
        available: true,
        current_version: update.current_version.clone(),
        available_version: update.version.clone(),
        notes: update.body.clone().filter(|s| !s.trim().is_empty()),
        pub_date: update.date.map(|d| d.to_string()),
    }
}

/// 按本机代理设置构造 updater。
///
/// 插件内部的 HTTP 客户端不读取 Windows 系统代理，开着 Clash 等代理客户端的用户
/// 直连 GitHub 容易超时，所以这里显式注入。
fn updater(app: &AppHandle) -> AppResult<Updater> {
    let mut builder = app.updater_builder();
    if let Some(proxy) = system_proxy() {
        log::info!("检查更新使用代理 {proxy}");
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|e| failed("初始化更新组件失败", e))
}

/// 记录日志并转换为前端可展示的错误
fn failed(what: &str, e: impl std::fmt::Display) -> AppError {
    log::warn!("{what}：{e}");
    AppError::internal(format!("{what}：{e}"))
}

/// 只查询是否有新版本，不下载。网络不通或清单校验失败时返回错误，
/// 由前端决定静默（启动检查）还是提示（手动检查）。
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> AppResult<UpdateCheck> {
    let found = updater(&app)?.check().await.map_err(|e| failed("检查更新失败", e))?;
    Ok(match found {
        Some(update) => update_check(&update),
        None => UpdateCheck {
            available: false,
            current_version: app.package_info().version.to_string(),
            available_version: String::new(),
            notes: None,
            pub_date: None,
        },
    })
}

/// 下载并校验签名，安装包留在内存中等待用户点击安装。
#[tauri::command]
pub async fn download_app_update(app: AppHandle, state: State<'_, UpdateState>) -> AppResult<UpdateCheck> {
    if state.downloading.swap(true, Ordering::AcqRel) {
        return Err(AppError::internal("更新包正在下载中"));
    }
    let _guard = DownloadGuard(&state.downloading);
    *state.prepared.lock().unwrap() = None;

    let update = updater(&app)?
        .check()
        .await
        .map_err(|e| failed("检查更新失败", e))?
        .ok_or_else(|| AppError::internal("当前已是最新版本"))?;
    let info = update_check(&update);

    let emitter = app.clone();
    let (mut downloaded, mut reported) = (0u64, 0u64);
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                if downloaded - reported >= PROGRESS_STEP || total == Some(downloaded) {
                    reported = downloaded;
                    let _ = emitter.emit(PROGRESS_EVENT, UpdateProgress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| failed("下载更新失败", e))?;

    *state.prepared.lock().unwrap() = Some(PreparedUpdate { update, bytes });
    Ok(info)
}

/// 安装已下载的更新包并重启。
///
/// Windows：插件拉起 NSIS 安装器（被动模式，装完自动启动新版本）后直接结束当前进程；
/// 结束前插件会调用 `cleanup_before_exit` 清掉托盘图标，不会残留旧图标。
/// macOS / Linux：原地替换后正常返回，再手动重启进入新版本。
#[tauri::command]
pub async fn install_app_update(app: AppHandle, state: State<'_, UpdateState>) -> AppResult<()> {
    let prepared = state
        .prepared
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| AppError::internal("安装包尚未下载完成"))?;

    if let Err(e) = prepared.update.install(&prepared.bytes) {
        // 安装失败时保留安装包，用户可以直接重试
        *state.prepared.lock().unwrap() = Some(prepared);
        return Err(failed("安装更新失败", e));
    }

    #[cfg(not(windows))]
    app.restart();
    // Windows 上安装成功时进程已经退出，走不到这里
    #[cfg(windows)]
    {
        let _ = app;
        Ok(())
    }
}

/// 读取本机代理地址：优先环境变量（跨平台惯例，也便于临时覆盖），其次 Windows 系统代理。
fn system_proxy() -> Option<Url> {
    for key in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Some(url) = std::env::var(key).ok().and_then(|v| parse_proxy_address(&v)) {
            return Some(url);
        }
    }
    windows_system_proxy()
}

/// 读取“Internet 选项”里的系统代理（Clash、v2rayN 等客户端写在这里）。
/// 只在这一处用到注册表，直接调用 `reg query`，不为它引入额外依赖。
#[cfg(windows)]
fn windows_system_proxy() -> Option<Url> {
    const INTERNET_SETTINGS: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    let enabled = reg_query(INTERNET_SETTINGS, "ProxyEnable")?;
    if enabled.trim_start_matches("0x").trim_start_matches('0').is_empty() {
        return None;
    }
    parse_proxy_address(&reg_query(INTERNET_SETTINGS, "ProxyServer")?)
}

#[cfg(not(windows))]
fn windows_system_proxy() -> Option<Url> {
    None
}

/// 取注册表单个值的数据部分；查不到或命令失败时返回 None。
#[cfg(windows)]
fn reg_query(path: &str, name: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;

    let output = std::process::Command::new("reg")
        .args(["query", path, "/v", name])
        .creation_flags(CREATE_NO_WINDOW.0)
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    parse_reg_value(&String::from_utf8_lossy(&output.stdout), name)
}

/// 从 `reg query` 输出中取出指定值的数据，输出形如 `    ProxyServer    REG_SZ    127.0.0.1:7890`。
/// 值本身可能含空格，所以跳过类型列后取剩余部分，而不是取最后一段。
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_reg_value(stdout: &str, name: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        let rest = line.trim().strip_prefix(name)?.trim_start();
        let (_, data) = rest.split_once(char::is_whitespace)?;
        let data = data.trim();
        (!data.is_empty()).then(|| data.to_string())
    })
}

/// 把代理地址归一化为带协议的 URL。兼容整串 `host:port`，
/// 以及按协议分开的 `http=host:port;https=host:port`（优先 https，其次 http）。
fn parse_proxy_address(raw: &str) -> Option<Url> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let address = if raw.contains('=') {
        let pick = |scheme: &str| {
            raw.split(';').find_map(|part| {
                let (k, v) = part.split_once('=')?;
                (k.trim().eq_ignore_ascii_case(scheme) && !v.trim().is_empty()).then(|| v.trim())
            })
        };
        pick("https").or_else(|| pick("http"))?.to_string()
    } else {
        raw.to_string()
    };
    let address = if address.contains("://") { address } else { format!("http://{address}") };
    Url::parse(&address).ok()
}

#[cfg(test)]
mod tests {
    use super::{parse_proxy_address, parse_reg_value};

    #[test]
    fn proxy_address_accepts_bare_host_port() {
        assert_eq!(parse_proxy_address("127.0.0.1:7890").unwrap().as_str(), "http://127.0.0.1:7890/");
    }

    #[test]
    fn proxy_address_keeps_explicit_scheme() {
        assert_eq!(parse_proxy_address("http://127.0.0.1:7890").unwrap().as_str(), "http://127.0.0.1:7890/");
    }

    #[test]
    fn proxy_address_prefers_https_then_http() {
        let both = parse_proxy_address("http=10.0.0.1:1080;https=10.0.0.2:1081;ftp=10.0.0.3:1082").unwrap();
        assert_eq!(both.as_str(), "http://10.0.0.2:1081/");
        let http_only = parse_proxy_address("ftp=10.0.0.3:1082;http=10.0.0.1:1080").unwrap();
        assert_eq!(http_only.as_str(), "http://10.0.0.1:1080/");
    }

    #[test]
    fn proxy_address_rejects_blank_and_unusable_values() {
        assert!(parse_proxy_address("").is_none());
        assert!(parse_proxy_address("   ").is_none());
        assert!(parse_proxy_address("ftp=10.0.0.3:1082").is_none());
    }

    #[test]
    fn reg_value_reads_data_column() {
        let stdout = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\r\n    ProxyServer    REG_SZ    127.0.0.1:7890\r\n\r\n";
        assert_eq!(parse_reg_value(stdout, "ProxyServer").as_deref(), Some("127.0.0.1:7890"));
    }

    #[test]
    fn reg_value_handles_dword_missing_name_and_spaces() {
        assert_eq!(parse_reg_value("    ProxyEnable    REG_DWORD    0x1\r\n", "ProxyEnable").as_deref(), Some("0x1"));
        assert!(parse_reg_value("    ProxyEnable    REG_DWORD    0x1\r\n", "ProxyServer").is_none());
        assert_eq!(
            parse_reg_value("    ProxyOverride    REG_SZ    <local>;*.cn; 10.0.0.1\r\n", "ProxyOverride").as_deref(),
            Some("<local>;*.cn; 10.0.0.1")
        );
    }
}

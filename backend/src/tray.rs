//! 系统托盘：主窗口点右上角关闭时隐藏到托盘，托盘右键弹出与应用同风格的菜单（打开主界面 / 退出）。
//!
//! Windows / macOS 的托盘菜单是一个无边框的小 WebView 窗口（前端 `tray.html`），首次右键时才创建，
//! 之后隐藏复用；菜单页加载完成后通过 `tray_menu_ready` 报到，再显示，避免首次弹出白屏。
//! Linux 的 AppIndicator 不上报托盘点击事件，只能挂系统原生菜单，菜单项与其他平台一致。

use tauri::{AppHandle, Window};

#[cfg(not(target_os = "linux"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_os = "linux"))]
use std::sync::Mutex;
#[cfg(not(target_os = "linux"))]
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri::{tray::TrayIconBuilder, Manager};

pub const MAIN_WINDOW: &str = "main";
pub const MENU_WINDOW: &str = "tray-menu";

/// 菜单窗口的逻辑尺寸，与 frontend/src/tray/TrayMenu.tsx 的布局高度严格一致
#[cfg(not(target_os = "linux"))]
const MENU_WIDTH: f64 = 196.0;
#[cfg(not(target_os = "linux"))]
const MENU_HEIGHT: f64 = 121.0;

/// 菜单页是否已加载完成
#[cfg(not(target_os = "linux"))]
static MENU_READY: AtomicBool = AtomicBool::new(false);
/// 菜单页加载完成前收到的右键位置，报到后在这里弹出
#[cfg(not(target_os = "linux"))]
static PENDING: Mutex<Option<PhysicalPosition<f64>>> = Mutex::new(None);

/// 创建托盘图标，在应用 setup 阶段调用。
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id("main").tooltip("port-helper");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    #[cfg(target_os = "linux")]
    let builder = {
        use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
        let open = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
        builder.menu(&menu).on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
    };

    #[cfg(not(target_os = "linux"))]
    let builder = builder.on_tray_icon_event(|tray, event| on_tray_event(tray.app_handle(), event));

    builder.build(app)?;
    Ok(())
}

/// 主窗口关闭请求：隐藏到托盘，不退出进程。
pub fn hide_main(window: &Window) {
    if let Err(e) = window.hide() {
        log::warn!("隐藏主窗口失败：{e}");
    }
    #[cfg(windows)]
    crate::memory::sync_with_window(window);
}

/// 从托盘恢复并聚焦主窗口（最小化、隐藏两种状态都能恢复）。
pub fn show_main(app: &AppHandle) {
    hide_menu(app);
    let Some(main) = app.get_webview_window(MAIN_WINDOW) else { return };
    let _ = main.unminimize();
    let _ = main.show();
    let _ = main.set_focus();
    #[cfg(windows)]
    crate::memory::sync_with_window(&main.as_ref().window());
}

pub fn hide_menu(app: &AppHandle) {
    if let Some(menu) = app.get_webview_window(MENU_WINDOW) {
        let _ = menu.hide();
    }
}

/// 菜单页加载完成：之后的右键直接弹出；加载期间有右键则现在弹出。
#[cfg(not(target_os = "linux"))]
pub fn menu_ready(app: &AppHandle) {
    MENU_READY.store(true, Ordering::Release);
    let pending = PENDING.lock().map(|mut p| p.take()).unwrap_or(None);
    if let (Some(cursor), Some(menu)) = (pending, app.get_webview_window(MENU_WINDOW)) {
        place_and_show(app, &menu, cursor);
    }
}

#[cfg(target_os = "linux")]
pub fn menu_ready(_app: &AppHandle) {}

#[cfg(not(target_os = "linux"))]
fn on_tray_event(app: &AppHandle, event: TrayIconEvent) {
    if let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. } = event {
        match button {
            MouseButton::Left => show_main(app),
            MouseButton::Right => show_menu(app, position),
            MouseButton::Middle => {}
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn show_menu(app: &AppHandle, cursor: PhysicalPosition<f64>) {
    match app.get_webview_window(MENU_WINDOW) {
        Some(menu) if MENU_READY.load(Ordering::Acquire) => place_and_show(app, &menu, cursor),
        existing => {
            if let Ok(mut p) = PENDING.lock() {
                *p = Some(cursor);
            }
            if existing.is_none() {
                if let Err(e) = build_menu_window(app) {
                    log::warn!("创建托盘菜单失败：{e}");
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn build_menu_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, MENU_WINDOW, WebviewUrl::App("tray.html".into()))
        .title("port-helper")
        .inner_size(MENU_WIDTH, MENU_HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
}

/// 以光标为锚点弹出菜单：默认在光标右上方展开，超出屏幕工作区（不含任务栏）时翻转到另一侧。
#[cfg(not(target_os = "linux"))]
fn place_and_show(app: &AppHandle, menu: &WebviewWindow, cursor: PhysicalPosition<f64>) {
    let monitor = app.monitor_from_point(cursor.x, cursor.y).ok().flatten();
    let scale = monitor.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let (w, h) = ((MENU_WIDTH * scale).round() as i32, (MENU_HEIGHT * scale).round() as i32);
    let (cx, cy) = (cursor.x.round() as i32, cursor.y.round() as i32);
    let (mut x, mut y) = (cx, cy - h);
    if let Some(area) = monitor.as_ref().map(|m| *m.work_area()) {
        let (left, top) = (area.position.x, area.position.y);
        let (right, bottom) = (left + area.size.width as i32, top + area.size.height as i32);
        if x + w > right {
            x = cx - w;
        }
        if y < top {
            y = cy;
        }
        x = x.clamp(left, (right - w).max(left));
        y = y.clamp(top, (bottom - h).max(top));
    }
    // 不在这里调 set_size：Windows 下无边框窗口设尺寸会把隐藏的标题栏高度算进去，内容区会变高；
    // 创建时的 inner_size 本身正确，跨不同缩放比例的显示器移动时系统也会按 DPI 自动缩放
    let _ = menu.set_position(PhysicalPosition::new(x, y));
    let _ = menu.show();
    let _ = menu.set_focus();
}

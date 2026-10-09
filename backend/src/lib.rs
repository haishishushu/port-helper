pub mod commands;
#[cfg(windows)]
pub mod console;
pub mod error;
#[cfg(windows)]
pub mod memory;
pub mod model;
pub mod net;
pub mod ops;
pub mod privilege;
pub mod process;
pub mod query;
pub mod tray;

use tauri::WindowEvent;
use tauri_plugin_log::{Target, TargetKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 本应用的命令都在阻塞线程池执行，异步工作线程用不了几个；
    // 默认运行时会按 CPU 核数创建工作线程，这里收紧以减少常驻线程与内存（perf-todo P2-1）
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .thread_name("port-helper-rt")
        .enable_all()
        .build()
        .expect("创建异步运行时失败");
    tauri::async_runtime::set(runtime.handle().clone());

    let log_level = if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info };
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log_level)
                .targets([Target::new(TargetKind::Stdout), Target::new(TargetKind::LogDir { file_name: None })])
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            tray::setup(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            // 右上角关闭（含 Alt+F4）只隐藏到托盘；真正退出走托盘菜单的“退出”
            (tray::MAIN_WINDOW, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                tray::hide_main(window);
            }
            // 托盘菜单点到别处即收起，与系统菜单行为一致
            (tray::MENU_WINDOW, WindowEvent::Focused(false)) => {
                let _ = window.hide();
            }
            #[cfg(windows)]
            (tray::MAIN_WINDOW, WindowEvent::Resized(_)) => memory::sync_with_window(window),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::query_port,
            commands::query_pid,
            commands::close_process,
            commands::kill_process,
            commands::get_privilege,
            commands::relaunch_as_admin,
            commands::tray_menu_ready,
            commands::tray_open_main,
            commands::tray_hide_menu,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("启动 port-helper 失败")
        .run(|_app, _event| {
            // macOS：主窗口隐藏到托盘后，点击程序坞图标也能恢复
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                tray::show_main(_app);
            }
        });
}

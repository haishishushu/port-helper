pub mod commands;
pub mod console;
pub mod error;
pub mod memory;
pub mod model;
pub mod net;
pub mod ops;
pub mod privilege;
pub mod process;
pub mod query;

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
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(_) = event {
                memory::sync_with_window(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::query_port,
            commands::query_pid,
            commands::close_process,
            commands::kill_process,
            commands::get_privilege,
            commands::relaunch_as_admin,
        ])
        .run(tauri::generate_context!())
        .expect("启动 port-helper 失败");
}

// 调试版和发布版都使用 GUI 子系统：任何启动方式（包括以管理员身份重新启动）都不会创建控制台窗口
#![windows_subsystem = "windows"]

fn main() {
    // 以 `--ctrl-c <pid>` 辅助模式启动时，发送 Ctrl+C 后直接退出，不启动界面
    #[cfg(windows)]
    port_helper_lib::console::run_helper_if_requested();
    // 开发时附加到启动它的终端（tauri dev），让日志仍能输出到终端；没有父控制台时什么也不做
    #[cfg(all(windows, debug_assertions))]
    port_helper_lib::console::attach_parent_console();
    port_helper_lib::run();
}

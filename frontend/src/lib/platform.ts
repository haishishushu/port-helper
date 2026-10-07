/**
 * 运行平台与按平台区分的文案。
 * 平台在构建时由 vite.config.ts 注入：各平台的安装包都在对应系统上构建，构建机平台即运行平台。
 */
export type Os = "windows" | "macos" | "linux";

export const OS: Os = __PLATFORM__ === "darwin" ? "macos" : __PLATFORM__ === "linux" ? "linux" : "windows";

interface PlatformText {
  /** 设置页“版本”右侧的平台标识 */
  label: string;
  /** 提权后的身份称呼：以{admin}身份重新启动、需要{admin}权限 */
  admin: string;
  /** 以管理员身份重新启动时系统会出现的确认方式 */
  elevatePrompt: string;
  /** 运行权限说明（普通权限时） */
  elevateReason: string;
  /** 正常关闭时发送的信号 */
  closeSignal: string;
  /** 正常关闭的方式说明 */
  closeHow: string;
  criticalTitle: string;
  serviceTag: string;
  serviceHint: (services: string[]) => string;
  /** 主题“跟随系统”的说明 */
  themeFollow: string;
  themeFollowHint: string;
}

const TEXT: Record<Os, PlatformText> = {
  windows: {
    label: "Windows x64",
    admin: "管理员",
    elevatePrompt: "Windows 会弹出“用户账户控制”请求确认。",
    elevateReason: "当前为普通权限。结束服务或 SYSTEM 进程需要管理员权限",
    closeSignal: "发送关闭信号（窗口关闭 / Ctrl+C）",
    closeHow: "有窗口的发送关闭消息，控制台进程发送 Ctrl+C",
    criticalTitle: "这是 Windows 关键进程",
    serviceTag: "Windows 服务",
    serviceHint: (s) => `这是 Windows 服务 ${s.join("、")}，建议在“服务”中停止。`,
    themeFollow: "随 Windows 切换",
    themeFollowHint: "选择“跟随系统”时，会随 Windows 的浅色/深色设置自动切换。",
  },
  macos: {
    label: "macOS",
    admin: "管理员",
    elevatePrompt: "macOS 会弹出密码框，需要输入管理员密码。",
    elevateReason: "当前为普通权限。查看或结束其他用户（如 root）的进程需要管理员权限",
    closeSignal: "发送关闭信号（SIGTERM）",
    closeHow: "向进程发送 SIGTERM 信号",
    criticalTitle: "这是 macOS 关键进程",
    serviceTag: "系统服务",
    serviceHint: (s) => `这是系统服务 ${s.join("、")}，建议通过 launchctl 停止。`,
    themeFollow: "随 macOS 切换",
    themeFollowHint: "选择“跟随系统”时，会随 macOS 的浅色/深色外观自动切换。",
  },
  linux: {
    label: "Linux x64",
    admin: "root",
    elevatePrompt: "系统会弹出认证框（pkexec），需要输入密码。",
    elevateReason: "当前为普通权限。查看或结束其他用户（如 root）的进程需要 root 权限",
    closeSignal: "发送关闭信号（SIGTERM）",
    closeHow: "向进程发送 SIGTERM 信号",
    criticalTitle: "这是 Linux 关键进程",
    serviceTag: "systemd 服务",
    serviceHint: (s) => `这是 systemd 服务 ${s.join("、")}，建议用 sudo systemctl stop ${s[0]} 停止。`,
    themeFollow: "随系统切换",
    themeFollowHint: "选择“跟随系统”时，会随系统的浅色/深色设置自动切换。",
  },
};

export const platform = TEXT[OS];

//! 应用识别与特殊占用者识别（纯逻辑）。
//!
//! 优先根据命令行和路径判断具体用途，而不是只看进程名。

use crate::model::{AppIcon, OccupantKind};

pub struct ClassifyInput<'a> {
    pub pid: u32,
    pub name: &'a str,
    pub exe_path: Option<&'a str>,
    pub command_line: Option<&'a str>,
    pub services: &'a [String],
    /// 端口查询时传入，用于区分 PID 4 是 HTTP.sys 还是 SMB
    pub port_hint: Option<u16>,
    pub self_pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub app_name: String,
    pub app_icon: AppIcon,
    pub kind: OccupantKind,
    pub actionable: bool,
    pub blocked_reason: Option<String>,
}

#[cfg(windows)]
const CRITICAL: &[&str] = &[
    "smss", "csrss", "wininit", "winlogon", "services", "lsass", "lsaiso", "svchost", "dwm", "fontdrvhost",
    "registry", "memory compression", "secure system",
];
#[cfg(target_os = "macos")]
const CRITICAL: &[&str] = &[
    "kernel_task", "launchd", "windowserver", "loginwindow", "mdnsresponder", "configd", "systemuiserver",
    "controlcenter", "rapportd", "sharingd", "netbiosd", "remoted", "apsd",
];
#[cfg(target_os = "linux")]
const CRITICAL: &[&str] = &[
    "init", "sshd", "dbus-daemon", "dbus-broker", "networkmanager", "avahi-daemon", "cupsd", "chronyd", "dnsmasq",
    "xorg", "xwayland", "gnome-shell", "kwin_x11", "kwin_wayland", "plasmashell",
];

#[cfg(windows)]
const FORWARDERS: &[(&str, &str, AppIcon)] = &[
    ("com.docker.backend", "Docker Desktop", AppIcon::Container),
    ("com.docker.proxy", "Docker Desktop", AppIcon::Container),
    ("vpnkit", "Docker Desktop", AppIcon::Container),
    ("dockerd", "Docker Engine", AppIcon::Container),
    ("wslrelay", "WSL 端口转发", AppIcon::Terminal),
    ("wslhost", "WSL 端口转发", AppIcon::Terminal),
    ("wslservice", "WSL 服务", AppIcon::Terminal),
];
#[cfg(target_os = "macos")]
const FORWARDERS: &[(&str, &str, AppIcon)] = &[
    ("com.docker.backend", "Docker Desktop", AppIcon::Container),
    ("com.docker.vpnkit", "Docker Desktop", AppIcon::Container),
    ("vpnkit-bridge", "Docker Desktop", AppIcon::Container),
];
#[cfg(target_os = "linux")]
const FORWARDERS: &[(&str, &str, AppIcon)] = &[
    ("docker-proxy", "Docker Engine", AppIcon::Container),
    ("rootlessport", "Podman", AppIcon::Container),
];
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
const CRITICAL: &[&str] = &[];
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
const FORWARDERS: &[(&str, &str, AppIcon)] = &[];

#[cfg(windows)]
const SMB_PORTS: &[u16] = &[137, 138, 139, 445];

/// 不对应真实进程或绝不能结束的 PID（各平台不同）。
#[cfg(windows)]
fn special_pid(input: &ClassifyInput) -> Option<Classification> {
    match input.pid {
        0 => Some(blocked(
            "系统（PID 0）",
            AppIcon::System,
            OccupantKind::Idle,
            "该记录由系统内核持有（如 TIME_WAIT），连接正在等待回收，无需也无法结束",
        )),
        4 => {
            let (app, reason) = match input.port_hint {
                Some(p) if SMB_PORTS.contains(&p) => (
                    "SMB 文件共享（Windows 内核）",
                    "PID 4 是 Windows 内核（System），该端口由文件共享服务使用，结束会导致系统崩溃",
                ),
                _ => (
                    "HTTP.sys（Windows HTTP 服务）",
                    "PID 4 是 Windows 内核（System），端口实际由 HTTP.sys 代为监听，请停止向它注册 URL 的服务",
                ),
            };
            Some(blocked(app, AppIcon::Server, OccupantKind::System, reason))
        }
        _ => None,
    }
}

#[cfg(not(windows))]
fn special_pid(input: &ClassifyInput) -> Option<Classification> {
    match input.pid {
        0 => Some(blocked(
            "未知进程",
            AppIcon::System,
            OccupantKind::Idle,
            if cfg!(target_os = "linux") {
                "读不到占用该端口的进程：可能属于其他用户（需要以 root 身份运行才能查看），也可能是内核持有、等待回收的连接（如 TIME_WAIT）"
            } else {
                "读不到占用该端口的进程：它可能属于其他用户，需要以管理员身份运行才能查看"
            },
        )),
        1 => Some(blocked(
            "系统初始化进程（PID 1）",
            AppIcon::System,
            OccupantKind::System,
            "PID 1 是系统的初始化进程（launchd / systemd），结束会导致系统崩溃",
        )),
        _ => None,
    }
}

/// 关键进程的展示名；Linux 上 systemd 自带的守护进程（systemd-resolved 等）一律算关键进程。
fn critical_app(name: &str, input: &ClassifyInput) -> Option<String> {
    let linux_systemd = cfg!(target_os = "linux") && name.starts_with("systemd");
    if !CRITICAL.contains(&name) && !linux_systemd {
        return None;
    }
    Some(if cfg!(windows) && name == "svchost" && !input.services.is_empty() {
        format!("服务宿主 · {}", input.services.join(", "))
    } else if cfg!(target_os = "macos") && name == "controlcenter" {
        "AirPlay 接收器（控制中心）".to_string()
    } else if cfg!(windows) {
        format!("Windows 关键进程（{}）", input.name)
    } else {
        format!("系统关键进程（{}）", input.name)
    })
}

pub fn classify(input: &ClassifyInput) -> Classification {
    // 统一按不带 .exe 的小写名称匹配
    let lower = input.name.to_ascii_lowercase();
    let name = lower.strip_suffix(".exe").unwrap_or(&lower);

    if let Some(c) = special_pid(input) {
        return c;
    }
    if input.pid == input.self_pid {
        return blocked("port-helper", AppIcon::Generic, OccupantKind::SelfProcess, "不能结束 port-helper 自身");
    }

    if let Some((_, app, icon)) = FORWARDERS.iter().find(|(n, _, _)| *n == name) {
        return allowed(app.to_string(), *icon, OccupantKind::Forwarder);
    }

    if let Some(app) = critical_app(name, input) {
        return allowed(app, AppIcon::System, OccupantKind::Critical);
    }

    let cmd = input.command_line.unwrap_or("").to_ascii_lowercase().replace('\\', "/");
    let exe = input.exe_path.unwrap_or("").to_ascii_lowercase().replace('\\', "/");
    let (app_name, app_icon) = recognize_app(name, &cmd, &exe, input.command_line.unwrap_or(""));
    let kind = if input.services.is_empty() { OccupantKind::Normal } else { OccupantKind::Service };
    allowed(app_name, app_icon, kind)
}

fn allowed(app_name: String, app_icon: AppIcon, kind: OccupantKind) -> Classification {
    Classification { app_name, app_icon, kind, actionable: true, blocked_reason: None }
}

fn blocked(app: &str, icon: AppIcon, kind: OccupantKind, reason: &str) -> Classification {
    Classification {
        app_name: app.to_string(),
        app_icon: icon,
        kind,
        actionable: false,
        blocked_reason: Some(reason.to_string()),
    }
}

fn has(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

fn recognize_app(name: &str, cmd: &str, exe: &str, raw_cmd: &str) -> (String, AppIcon) {
    let pick = |s: &str, icon| (s.to_string(), icon);
    // python3、python3.12 等都按 python 处理
    let key = if name.starts_with("python") { "python" } else { name };
    // 末尾补空格，让 `/vite `、`/next ` 这类规则也能匹配以脚本路径结尾的命令行（如 `node .bin/vite`）
    let cmd = &format!("{cmd} ");
    match key {
        "node" => {
            if has(cmd, &["/vite/", "vite.js", " vite", "/vite "]) {
                pick("Vite 开发服务器", AppIcon::Web)
            } else if has(cmd, &["/next/", "next dev", "next start", "/next "]) {
                pick("Next.js", AppIcon::Web)
            } else if has(cmd, &["/nuxt", "nuxi"]) {
                pick("Nuxt", AppIcon::Web)
            } else if has(cmd, &["react-scripts"]) {
                pick("Create React App", AppIcon::Web)
            } else if has(cmd, &["webpack"]) {
                pick("Webpack Dev Server", AppIcon::Web)
            } else if has(cmd, &["@nestjs", "nest start"]) {
                pick("NestJS", AppIcon::Server)
            } else if has(cmd, &["nodemon"]) {
                pick("nodemon", AppIcon::Web)
            } else {
                pick("Node.js", AppIcon::Web)
            }
        }
        "java" | "javaw" => {
            if has(cmd, &["org.springframework.boot", "spring-boot"]) {
                pick("Spring Boot", AppIcon::Java)
            } else if has(exe, &["jetbrains", "intellij"]) || has(cmd, &["jetbrains"]) {
                pick("IntelliJ IDEA", AppIcon::Java)
            } else if has(cmd, &["catalina", "tomcat"]) {
                pick("Tomcat", AppIcon::Java)
            } else if let Some(jar) = jar_name(raw_cmd) {
                (format!("Java · {jar}"), AppIcon::Java)
            } else {
                pick("Java", AppIcon::Java)
            }
        }
        "python" | "py" => {
            if has(cmd, &["uvicorn"]) {
                pick("FastAPI (Uvicorn)", AppIcon::Python)
            } else if has(cmd, &["fastapi"]) {
                pick("FastAPI", AppIcon::Python)
            } else if has(cmd, &["flask"]) {
                pick("Flask", AppIcon::Python)
            } else if has(cmd, &["manage.py runserver", "django"]) {
                pick("Django", AppIcon::Python)
            } else if has(cmd, &["gunicorn"]) {
                pick("Gunicorn", AppIcon::Python)
            } else if has(cmd, &["http.server"]) {
                pick("Python HTTP Server", AppIcon::Python)
            } else {
                pick("Python", AppIcon::Python)
            }
        }
        "mysqld" => pick("MySQL", AppIcon::Database),
        "redis-server" => pick("Redis", AppIcon::Database),
        "postgres" => pick("PostgreSQL", AppIcon::Database),
        "mongod" => pick("MongoDB", AppIcon::Database),
        "nginx" => pick("Nginx", AppIcon::Server),
        "httpd" | "apache2" => pick("Apache httpd", AppIcon::Server),
        _ => (name.to_string(), AppIcon::Generic),
    }
}

/// 从 `-jar xxx.jar` 中取出 jar 文件名。
fn jar_name(raw_cmd: &str) -> Option<String> {
    let mut parts = raw_cmd.split_whitespace();
    while let Some(p) = parts.next() {
        if p == "-jar" {
            let jar = parts.next()?.trim_matches('"');
            let file = jar.rsplit(['/', '\\']).next().unwrap_or(jar);
            return Some(file.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(pid: u32, name: &str, cmd: &str, services: &[String], port: Option<u16>) -> Classification {
        classify(&ClassifyInput {
            pid,
            name,
            exe_path: None,
            command_line: Some(cmd),
            services,
            port_hint: port,
            self_pid: 99_999,
        })
    }

    #[test]
    fn recognizes_dev_servers_by_command_line() {
        let vite = run(1, "node.exe", r"node D:\app\node_modules\vite\bin\vite.js --port 5173", &[], None);
        assert_eq!(vite.app_name, "Vite 开发服务器");
        let spring = run(2, "java.exe", "java -cp x org.springframework.boot.loader.JarLauncher", &[], None);
        assert_eq!(spring.app_name, "Spring Boot");
        let jar = run(3, "java.exe", r#"java -jar "D:\code\shop-api.jar""#, &[], None);
        assert_eq!(jar.app_name, "Java · shop-api.jar");
        let api = run(40, "python.exe", "python -m uvicorn main:app", &[], None);
        assert_eq!(api.app_name, "FastAPI (Uvicorn)");
        assert_eq!(run(5, "mysqld.exe", "", &[], None).app_name, "MySQL");
    }

    #[test]
    #[cfg(windows)]
    fn protects_special_pids() {
        let idle = run(0, "System Idle Process", "", &[], Some(5173));
        assert_eq!(idle.kind, OccupantKind::Idle);
        assert!(!idle.actionable);
        let http = run(4, "System", "", &[], Some(80));
        assert_eq!(http.app_name, "HTTP.sys（Windows HTTP 服务）");
        assert!(!http.actionable);
        let smb = run(4, "System", "", &[], Some(445));
        assert!(smb.app_name.starts_with("SMB"));
        let me = run(99_999, "port-helper.exe", "", &[], None);
        assert_eq!(me.kind, OccupantKind::SelfProcess);
    }

    #[test]
    #[cfg(windows)]
    fn marks_critical_service_and_forwarders() {
        let svc = vec!["Dnscache".to_string()];
        let host = run(1000, "svchost.exe", "", &svc, None);
        assert_eq!(host.kind, OccupantKind::Critical);
        assert_eq!(host.app_name, "服务宿主 · Dnscache");
        let mysql = run(5216, "mysqld.exe", "", &["MySQL80".to_string()], None);
        assert_eq!(mysql.kind, OccupantKind::Service);
        let docker = run(7000, "com.docker.backend.exe", "", &[], None);
        assert_eq!(docker.kind, OccupantKind::Forwarder);
        assert!(docker.actionable);
    }

    #[test]
    fn recognizes_unix_process_names() {
        assert_eq!(run(6, "node", "node ./node_modules/.bin/vite", &[], None).app_name, "Vite 开发服务器");
        assert_eq!(run(7, "python3.12", "python3 -m http.server 8000", &[], None).app_name, "Python HTTP Server");
        assert_eq!(run(8, "redis-server", "", &[], None).app_name, "Redis");
        assert_eq!(run(9, "my-tool", "", &[], None).app_name, "my-tool");
    }

    #[test]
    #[cfg(unix)]
    fn protects_unix_special_pids() {
        let unknown = run(0, "未知进程", "", &[], Some(8080));
        assert_eq!(unknown.kind, OccupantKind::Idle);
        assert!(!unknown.actionable);
        let init = run(1, "launchd", "", &[], None);
        assert_eq!(init.kind, OccupantKind::System);
        assert!(!init.actionable);
        let me = run(99_999, "port-helper", "", &[], None);
        assert_eq!(me.kind, OccupantKind::SelfProcess);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn marks_linux_critical_service_and_forwarders() {
        assert_eq!(run(300, "systemd-resolved", "", &[], None).kind, OccupantKind::Critical);
        assert_eq!(run(301, "sshd", "", &[], None).kind, OccupantKind::Critical);
        let nginx = run(302, "nginx", "", &["nginx".to_string()], None);
        assert_eq!(nginx.kind, OccupantKind::Service);
        assert_eq!(run(303, "docker-proxy", "", &[], None).kind, OccupantKind::Forwarder);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn marks_macos_critical_and_forwarders() {
        let airplay = run(400, "ControlCenter", "", &[], None);
        assert_eq!(airplay.kind, OccupantKind::Critical);
        assert_eq!(airplay.app_name, "AirPlay 接收器（控制中心）");
        assert_eq!(run(401, "com.docker.backend", "", &[], None).kind, OccupantKind::Forwarder);
    }
}

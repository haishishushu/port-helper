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

const CRITICAL: &[&str] = &[
    "smss.exe", "csrss.exe", "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe",
    "lsaiso.exe", "svchost.exe", "dwm.exe", "fontdrvhost.exe", "registry", "memory compression",
    "secure system",
];

const FORWARDERS: &[(&str, &str, AppIcon)] = &[
    ("com.docker.backend.exe", "Docker Desktop", AppIcon::Container),
    ("com.docker.proxy.exe", "Docker Desktop", AppIcon::Container),
    ("vpnkit.exe", "Docker Desktop", AppIcon::Container),
    ("dockerd.exe", "Docker Engine", AppIcon::Container),
    ("wslrelay.exe", "WSL 端口转发", AppIcon::Terminal),
    ("wslhost.exe", "WSL 端口转发", AppIcon::Terminal),
    ("wslservice.exe", "WSL 服务", AppIcon::Terminal),
];

const SMB_PORTS: &[u16] = &[137, 138, 139, 445];

pub fn classify(input: &ClassifyInput) -> Classification {
    let name = input.name.to_ascii_lowercase();

    if input.pid == 0 {
        return blocked(
            "系统（PID 0）",
            AppIcon::System,
            OccupantKind::Idle,
            "该记录由系统内核持有（如 TIME_WAIT），连接正在等待回收，无需也无法结束",
        );
    }
    if input.pid == 4 {
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
        return blocked(app, AppIcon::Server, OccupantKind::System, reason);
    }
    if input.pid == input.self_pid {
        return blocked("port-helper", AppIcon::Generic, OccupantKind::SelfProcess, "不能结束 port-helper 自身");
    }

    if let Some((_, app, icon)) = FORWARDERS.iter().find(|(n, _, _)| *n == name) {
        return allowed(app.to_string(), *icon, OccupantKind::Forwarder);
    }

    if CRITICAL.contains(&name.as_str()) {
        let app = if name == "svchost.exe" && !input.services.is_empty() {
            format!("服务宿主 · {}", input.services.join(", "))
        } else {
            format!("Windows 关键进程（{}）", input.name)
        };
        return allowed(app, AppIcon::System, OccupantKind::Critical);
    }

    let cmd = input.command_line.unwrap_or("").to_ascii_lowercase().replace('\\', "/");
    let exe = input.exe_path.unwrap_or("").to_ascii_lowercase().replace('\\', "/");
    let (app_name, app_icon) = recognize_app(&name, &cmd, &exe, input.command_line.unwrap_or(""));
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
    match name {
        "node.exe" => {
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
        "java.exe" | "javaw.exe" => {
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
        "python.exe" | "pythonw.exe" | "py.exe" => {
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
        "mysqld.exe" => pick("MySQL", AppIcon::Database),
        "redis-server.exe" => pick("Redis", AppIcon::Database),
        "postgres.exe" => pick("PostgreSQL", AppIcon::Database),
        "mongod.exe" => pick("MongoDB", AppIcon::Database),
        "nginx.exe" => pick("Nginx", AppIcon::Server),
        "httpd.exe" => pick("Apache httpd", AppIcon::Server),
        _ => {
            let base = name.strip_suffix(".exe").unwrap_or(name);
            (base.to_string(), AppIcon::Generic)
        }
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
}

//! PID → 该进程托管的服务名（Windows 服务 / systemd 服务）。
//!
//! 服务名只是辅助信息，读取失败时返回空表，不影响查询。

use std::collections::HashMap;

/// 读取 `pids` 对应的服务名。Windows 一次枚举全部运行中的服务（忽略 `pids`）；
/// Linux 逐个读取 `/proc/<pid>/cgroup`；macOS 不识别服务。
pub fn service_map(pids: &[u32]) -> HashMap<u32, Vec<String>> {
    #[cfg(windows)]
    let map = {
        let _ = pids;
        windows_impl::service_map()
    };
    #[cfg(target_os = "linux")]
    let map: HashMap<u32, Vec<String>> = pids
        .iter()
        .filter_map(|&pid| {
            let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
            Some((pid, vec![systemd_unit(&text)?]))
        })
        .collect();
    #[cfg(not(any(windows, target_os = "linux")))]
    let map = {
        let _ = pids;
        HashMap::new()
    };
    map
}

/// 从 cgroup 内容中取出系统级 systemd 服务名（`/system.slice/nginx.service` → `nginx`）。
/// 用户会话下的进程（`/user.slice/...`）不算服务。
#[cfg(any(target_os = "linux", test))]
fn systemd_unit(cgroup: &str) -> Option<String> {
    cgroup.lines().find_map(|line| {
        // cgroup v2：`0::/system.slice/x.service`；v1：`1:name=systemd:/system.slice/x.service`
        let path = line.splitn(3, ':').nth(2)?;
        let rest = path.strip_prefix("/system.slice/")?;
        rest.split('/').find_map(|part| part.strip_suffix(".service")).map(str::to_string)
    })
}

#[cfg(windows)]
mod windows_impl {
    use std::collections::HashMap;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_MORE_DATA;
    use windows::Win32::System::Services::{
        CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, ENUM_SERVICE_STATUS_PROCESSW,
        SC_ENUM_PROCESS_INFO, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_ACTIVE, SERVICE_WIN32,
    };

    /// 读取正在运行的服务；失败时返回空表。
    pub fn service_map() -> HashMap<u32, Vec<String>> {
        let mut map: HashMap<u32, Vec<String>> = HashMap::new();
        unsafe {
            let Ok(scm) = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ENUMERATE_SERVICE) else {
                return map;
            };
            let mut needed = 0u32;
            let mut returned = 0u32;
            let mut resume = 0u32;
            // u64 缓冲保证结构体中指针字段的对齐
            let mut buf: Vec<u64> = vec![0; 64 * 1024 / 8];
            loop {
                let bytes = std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u8>(), buf.len() * 8);
                let result = EnumServicesStatusExW(
                    scm,
                    SC_ENUM_PROCESS_INFO,
                    SERVICE_WIN32,
                    SERVICE_ACTIVE,
                    Some(bytes),
                    &mut needed,
                    &mut returned,
                    Some(&mut resume),
                    PCWSTR::null(),
                );
                let entries = std::slice::from_raw_parts(
                    buf.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>(),
                    returned as usize,
                );
                for e in entries {
                    let pid = e.ServiceStatusProcess.dwProcessId;
                    if pid != 0 {
                        if let Ok(name) = e.lpServiceName.to_string() {
                            map.entry(pid).or_default().push(name);
                        }
                    }
                }
                match result {
                    Ok(()) => break,
                    Err(err) if err.code() == ERROR_MORE_DATA.to_hresult() => {
                        if needed as usize > buf.len() * 8 {
                            buf = vec![0; needed as usize / 8 + 1];
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = CloseServiceHandle(scm);
        }
        for names in map.values_mut() {
            names.sort();
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::systemd_unit;

    #[test]
    fn parses_systemd_service_from_cgroup() {
        assert_eq!(systemd_unit("0::/system.slice/nginx.service
").as_deref(), Some("nginx"));
        assert_eq!(
            systemd_unit("12:pids:/
1:name=systemd:/system.slice/system-getty.slice/getty@tty1.service").as_deref(),
            Some("getty@tty1")
        );
        assert_eq!(systemd_unit("0::/user.slice/user-1000.slice/user@1000.service/app.slice/x.scope"), None);
        assert_eq!(systemd_unit("0::/init.scope"), None);
    }
}
